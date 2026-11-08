namespace Homelab.Vault.Setup;

internal sealed class InstallationBackup : IDisposable
{
    private readonly string? directory;
    public bool Exists => directory != null;
    public InstallationBackup()
    {
        if (!File.Exists(SetupOperations.Executable)) return;
        var data = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "HomelabVaultAgent");
        directory = Path.Combine(data, "setup-backup-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);
        foreach (var folder in Directory.EnumerateDirectories(SetupOperations.Root, "*", SearchOption.AllDirectories))
            if ((File.GetAttributes(folder) & FileAttributes.ReparsePoint) != 0) throw new IOException("Sauvegarde : dossier lié refusé.");
        foreach (var file in Directory.EnumerateFiles(SetupOperations.Root, "*", SearchOption.AllDirectories)) {
            if ((File.GetAttributes(file) & FileAttributes.ReparsePoint) != 0) throw new IOException("Sauvegarde : fichier lié refusé.");
            var target = Path.Combine(directory, Path.GetRelativePath(SetupOperations.Root, file));
            Directory.CreateDirectory(Path.GetDirectoryName(target)!); File.Copy(file, target, true);
        }
    }
    public void Restore()
    {
        if (directory == null) return;
        foreach (var file in Directory.EnumerateFiles(directory, "*", SearchOption.AllDirectories)) {
            var target = Path.GetFullPath(Path.Combine(SetupOperations.Root, Path.GetRelativePath(directory, file)));
            if (!target.StartsWith(SetupOperations.Root + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)) throw new IOException("Chemin de restauration refusé.");
            Directory.CreateDirectory(Path.GetDirectoryName(target)!); File.Copy(file, target, true);
        }
    }
    public void Dispose()
    {
        if (directory == null) return;
        var target = Path.GetFullPath(directory);
        var parent = Path.GetFullPath(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "HomelabVaultAgent"));
        if (Path.GetDirectoryName(target) != parent || !Path.GetFileName(target).StartsWith("setup-backup-", StringComparison.Ordinal) ||
            (File.GetAttributes(target) & FileAttributes.ReparsePoint) != 0) throw new IOException("Chemin de nettoyage refusé.");
        Directory.Delete(target, true);
    }
}
