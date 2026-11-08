using System.Net.Http;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Homelab.Vault;

internal sealed record ReleaseCandidate(Version Version, string Tag, bool Experimental, string InstallerUrl, string HashUrl, string FileName);
internal static class UpdateService
{
    private const string Repository = "https://github.com/MonsieurZed/VaultAgent/releases/download/";
    private static readonly HttpClient Http = CreateClient();
    private static HttpClient CreateClient() {
        var client = new HttpClient { Timeout = TimeSpan.FromMinutes(5) };
        client.DefaultRequestHeaders.UserAgent.ParseAdd("VaultAgent/2.1.1");
        client.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json"); return client;
    }
    public static Version Current => typeof(UpdateService).Assembly.GetName().Version!;
    public static ReleaseCandidate? Select(JsonElement releases, string channel)
    {
        if (channel is not ("stable" or "experimental")) throw new ArgumentException("Invalid channel");
        var candidates = new List<ReleaseCandidate>();
        foreach (var release in releases.EnumerateArray()) {
            if (release.GetProperty("draft").GetBoolean()) continue;
            var experimental = release.GetProperty("prerelease").GetBoolean();
            if (channel == "stable" && experimental) continue;
            var tag = release.GetProperty("tag_name").GetString()!;
            var match = Regex.Match(tag, @"^v?(\d+\.\d+\.\d+)(?:-[A-Za-z0-9.-]+)?$");
            if (!match.Success || !Version.TryParse(match.Groups[1].Value, out var version)) continue;
            if (tag.Contains('-') && !experimental) continue;
            var file = "VaultAgent-Setup-" + version.ToString(3) + "-x64.exe";
            var assets = release.GetProperty("assets").EnumerateArray().ToArray();
            string? Find(string name) => assets.FirstOrDefault(asset => asset.GetProperty("name").GetString() == name).ValueKind == JsonValueKind.Undefined ? null :
                assets.First(asset => asset.GetProperty("name").GetString() == name).GetProperty("browser_download_url").GetString();
            var installer = Find(file); var hash = Find(file + ".sha256");
            var prefix = Repository + Uri.EscapeDataString(tag) + "/";
            if (installer != prefix + file || hash != prefix + file + ".sha256") continue;
            candidates.Add(new(version, tag, experimental, installer, hash, file));
        }
        return candidates.OrderByDescending(value => value, Comparer<ReleaseCandidate>.Create(Compare)).FirstOrDefault();
    }
    private static int Compare(ReleaseCandidate left, ReleaseCandidate right) {
        var version = left.Version.CompareTo(right.Version); if (version != 0) return version;
        if (left.Experimental != right.Experimental) return left.Experimental ? -1 : 1;
        string[] Parts(string tag) => tag.Contains('-') ? tag[(tag.IndexOf('-') + 1)..].Split('.') : Array.Empty<string>();
        var a = Parts(left.Tag); var b = Parts(right.Tag);
        if (a.Length == 0 || b.Length == 0) return a.Length == b.Length ? 0 : a.Length == 0 ? 1 : -1;
        for (var i = 0; i < Math.Min(a.Length, b.Length); i++) {
            var numberA = long.TryParse(a[i], out var x); var numberB = long.TryParse(b[i], out var y);
            var comparison = numberA && numberB ? x.CompareTo(y) : numberA != numberB ? numberA ? -1 : 1 : string.CompareOrdinal(a[i], b[i]);
            if (comparison != 0) return comparison;
        }
        return a.Length.CompareTo(b.Length);
    }
    public static async Task<ReleaseCandidate?> CheckAsync(string channel, CancellationToken token) {
        // Deux pages publiques, sans jeton ni informations sur le coffre.
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(token); timeout.CancelAfter(TimeSpan.FromSeconds(25));
        var releases = new List<JsonElement>();
        for (var page = 1; page <= 2; page++) {
            using var response = await Http.GetAsync("https://api.github.com/repos/MonsieurZed/VaultAgent/releases?per_page=100&page=" + page, timeout.Token);
            response.EnsureSuccessStatusCode();
            using var json = JsonDocument.Parse(await response.Content.ReadAsStringAsync(timeout.Token));
            var entries = json.RootElement.EnumerateArray().ToArray(); releases.AddRange(entries.Select(value => value.Clone()));
            if (entries.Length < 100) break;
        }
        return Select(JsonSerializer.SerializeToElement(releases), channel);
    }
    public static async Task<string> DownloadAsync(ReleaseCandidate release, CancellationToken token)
    {
        var prefix = Repository + Uri.EscapeDataString(release.Tag) + "/";
        if (release.InstallerUrl != prefix + release.FileName || release.HashUrl != prefix + release.FileName + ".sha256" || !Regex.IsMatch(release.FileName, @"^VaultAgent-Setup-\d+\.\d+\.\d+-x64\.exe$")) throw new InvalidDataException("Invalid release source");
        var hashText = await Http.GetStringAsync(release.HashUrl, token);
        if (hashText.Length > 1024) throw new InvalidDataException("Invalid checksum");
        var match = Regex.Match(hashText.Trim(), @"^([a-fA-F0-9]{64})\s+\*?" + Regex.Escape(release.FileName) + "$", RegexOptions.CultureInvariant);
        if (!match.Success) throw new InvalidDataException("Invalid checksum");
        var directory = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "VaultAgent", "Updates", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory); var path = Path.Combine(directory, release.FileName);
        try {
            using var response = await Http.GetAsync(release.InstallerUrl, HttpCompletionOption.ResponseHeadersRead, token); response.EnsureSuccessStatusCode();
            if (response.Content.Headers.ContentLength > 300 * 1024 * 1024) throw new InvalidDataException("Installer too large");
            await using (var input = await response.Content.ReadAsStreamAsync(token)) await using (var output = File.Create(path)) {
                var buffer = new byte[81920]; long total = 0; int count;
                while ((count = await input.ReadAsync(buffer, token)) != 0) {
                    total += count; if (total > 300 * 1024 * 1024) throw new InvalidDataException("Installer too large");
                    await output.WriteAsync(buffer.AsMemory(0, count), token);
                }
            }
            await using var file = File.OpenRead(path);
            if (!Convert.ToHexString(await SHA256.HashDataAsync(file, token)).Equals(match.Groups[1].Value, StringComparison.OrdinalIgnoreCase)) throw new InvalidDataException("SHA256 mismatch");
            return path;
        }
        catch { if (File.Exists(path)) File.Delete(path); throw; }
    }
}
