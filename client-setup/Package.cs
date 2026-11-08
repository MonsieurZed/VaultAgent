using System.IO.Compression;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;

namespace Homelab.Vault.ClientSetup;
internal static class Package
{
    private static ZipArchive Open() => new(Assembly.GetExecutingAssembly().GetManifestResourceStream("Client.Payload.zip")!, ZipArchiveMode.Read);
    private static string Target(string root, string name) {
        var target = Path.GetFullPath(Path.Combine(root, name.Replace('/', Path.DirectorySeparatorChar)));
        if (!target.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase) || Path.IsPathRooted(name)) throw new IOException("Chemin du paquet refuse."); return target;
    }
    public static void Verify() {
        using var archive = Open(); using var manifestStream = archive.GetEntry("client-manifest.json")!.Open(); using var json = JsonDocument.Parse(manifestStream);
        var files = json.RootElement.Deserialize<Dictionary<string, string>>()!;
        if (!files.ContainsKey("VaultAgent-Client.exe") || !files.ContainsKey("uninstall.ps1") || files.ContainsKey("VaultAgent.exe")) throw new IOException("Paquet client invalide.");
        var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var entry in archive.Entries) {
            _ = Target(Install.Root, entry.FullName); if (!names.Add(entry.FullName)) throw new IOException("Fichier duplique.");
            if (entry.FullName.EndsWith('/') || entry.FullName == "client-manifest.json") continue;
            using var input = entry.Open();
            if (!files.TryGetValue(entry.FullName, out var hash) || !Convert.ToHexString(SHA256.HashData(input)).Equals(hash, StringComparison.OrdinalIgnoreCase)) throw new IOException("Empreinte invalide.");
        }
        if (files.Keys.Any(name => !names.Contains(name))) throw new IOException("Fichier absent.");
    }
    public static void Extract() {
        using var archive = Open();
        foreach (var entry in archive.Entries) {
            var path = Target(Install.Root, entry.FullName);
            if (entry.FullName.EndsWith('/')) { Directory.CreateDirectory(path); continue; }
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            for (var parent = new DirectoryInfo(Path.GetDirectoryName(path)!); parent != null && parent.FullName.StartsWith(Install.Root, StringComparison.OrdinalIgnoreCase); parent = parent.Parent)
                if ((parent.Attributes & FileAttributes.ReparsePoint) != 0) throw new IOException("Dossier lie refuse.");
            if (File.Exists(path) && (File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0) throw new IOException("Fichier lie refuse.");
            using var input = entry.Open(); using var output = File.Create(path); input.CopyTo(output);
        }
    }
}
