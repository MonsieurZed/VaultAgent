using System.IO.Compression;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;

namespace Homelab.Vault.Setup;

internal static class Payload
{
    private static ZipArchive Open() => new(Assembly.GetExecutingAssembly().GetManifestResourceStream("VaultAgent.Payload.zip") ?? throw new IOException("Paquet absent."), ZipArchiveMode.Read);
    private static Dictionary<string, string> Manifest(ZipArchive archive)
    {
        using var entry = archive.GetEntry("package-manifest.json")?.Open() ?? throw new IOException("Manifeste absent.");
        using var json = JsonDocument.Parse(entry);
        return json.RootElement.GetProperty("files").Deserialize<Dictionary<string, string>>() ?? throw new IOException("Manifeste invalide.");
    }
    private static string Target(string root, string name)
    {
        var target = Path.GetFullPath(Path.Combine(root, name.Replace('/', Path.DirectorySeparatorChar)));
        if (!target.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase) || Path.IsPathRooted(name)) throw new IOException("Chemin du paquet refusé.");
        return target;
    }
    public static void Verify()
    {
        using var archive = Open(); var manifest = Manifest(archive);
        if (manifest.Count < 10 || !manifest.ContainsKey("VaultAgent.exe") || !manifest.ContainsKey("setup/uninstall.ps1")) throw new IOException("Paquet incomplet.");
        var entries = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var entry in archive.Entries) {
            _ = Target(SetupOperations.Root, entry.FullName);
            if (!entries.Add(entry.FullName)) throw new IOException("Entrée dupliquée.");
            if (entry.FullName.EndsWith('/')) continue;
            if (entry.FullName == "package-manifest.json") continue;
            if (!manifest.TryGetValue(entry.FullName, out var expected)) throw new IOException("Fichier hors manifeste.");
            using var input = entry.Open();
            if (!Convert.ToHexString(SHA256.HashData(input)).Equals(expected, StringComparison.OrdinalIgnoreCase)) throw new IOException("Empreinte du paquet invalide.");
        }
        if (manifest.Keys.Any(name => !entries.Contains(name))) throw new IOException("Fichier manquant.");
    }
    public static void Extract()
    {
        using var archive = Open();
        foreach (var entry in archive.Entries) {
            var target = Target(SetupOperations.Root, entry.FullName);
            if (entry.FullName.EndsWith('/')) { Directory.CreateDirectory(target); continue; }
            Directory.CreateDirectory(Path.GetDirectoryName(target)!);
            for (var parent = new DirectoryInfo(Path.GetDirectoryName(target)!); parent != null && parent.FullName.StartsWith(SetupOperations.Root, StringComparison.OrdinalIgnoreCase); parent = parent.Parent)
                if ((parent.Attributes & FileAttributes.ReparsePoint) != 0) throw new IOException("Dossier lié refusé.");
            if (File.Exists(target) && (File.GetAttributes(target) & FileAttributes.ReparsePoint) != 0) throw new IOException("Fichier lié refusé.");
            using var input = entry.Open(); using var output = new FileStream(target, FileMode.Create, FileAccess.Write, FileShare.None); input.CopyTo(output);
        }
    }
}
