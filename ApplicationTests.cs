using System.Text.Json;

namespace Homelab.Vault;

internal static class ApplicationTests
{
    public static void Run()
    {
        var releases = JsonSerializer.SerializeToElement(new[] {
            Release("v2.0.1", false), Release("v2.1.1-beta.1", true), Release("v2.1.1-beta.10", true), Release("v2.1.1-beta.2", true), Release("v99.0.0", false, draft: true),
            Release("v50.0.0", false, malicious: true), Release("v40.0.0-beta", false), Release("v30.0.0", false, missingHash: true)
        });
        if (UpdateService.Select(releases, "stable")?.Tag != "v2.0.1") throw new Exception("Canal stable invalide");
        if (UpdateService.Select(releases, "experimental")?.Tag != "v2.1.1-beta.10") throw new Exception("Canal experimental invalide");
        var empty = JsonSerializer.SerializeToElement(Array.Empty<object>());
        if (UpdateService.Select(empty, "stable") != null) throw new Exception("Canal vide invalide");
        foreach (var pair in UiText.Entries) {
            if (pair.Value.Length != 2 || pair.Value.Any(string.IsNullOrWhiteSpace)) throw new Exception("Traduction incomplete : " + pair.Key);
            if (UiText.Translate(pair.Key, "fr") != pair.Key || UiText.Translate(pair.Key, "en") != pair.Value[0] || UiText.Translate(pair.Key, "es") != pair.Value[1]) throw new Exception("Traduction invalide");
        }
        if (UiText.Translate("Compte", "es") != "Cuenta") throw new Exception("Espagnol indisponible");
    }
    private static object Release(string tag, bool prerelease, bool draft = false, bool malicious = false, bool missingHash = false) {
        var version = tag.TrimStart('v').Split('-')[0]; var file = "VaultAgent-Setup-" + version + "-x64.exe";
        var prefix = malicious ? "https://example.invalid/" : "https://github.com/MonsieurZed/VaultAgent/releases/download/" + tag + "/";
        var assets = new List<object> { new { name = file, browser_download_url = prefix + file } };
        if (!missingHash) assets.Add(new { name = file + ".sha256", browser_download_url = prefix + file + ".sha256" });
        return new { tag_name = tag, prerelease, draft, assets };
    }
}
