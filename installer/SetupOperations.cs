using System.Diagnostics;
using System.Security.AccessControl;
using System.Security.Principal;
using System.ServiceProcess;
using System.Text.Json;
using Microsoft.Win32;

namespace Homelab.Vault.Setup;

internal static class SetupOperations
{
    public static string Root => Path.GetFullPath(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "HomelabVaultAgent"));
    public static string Executable => Path.Combine(Root, "VaultAgent.exe");
    private const string Service = "HomelabVaultAgent";
    public const string UninstallKey = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\HomelabVaultAgent";
    public static void WriteResult(bool success, string message)
    {
        try {
            if (!Directory.Exists(Root) || (File.GetAttributes(Root) & FileAttributes.ReparsePoint) != 0) return;
            var owner = new DirectoryInfo(Root).GetAccessControl().GetOwner(typeof(SecurityIdentifier))?.Value;
            if (owner is not ("S-1-5-18" or "S-1-5-32-544")) return;
            var file = Path.Combine(Root, "setup-result.json");
            if (File.Exists(file) && (File.GetAttributes(file) & FileAttributes.ReparsePoint) != 0) return;
            File.WriteAllText(file, JsonSerializer.Serialize(new { success, message, date = DateTimeOffset.UtcNow }));
        } catch { }
    }
    private static int Run(string file, params string[] args)
    {
        var info = new ProcessStartInfo(file) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true };
        foreach (var argument in args) info.ArgumentList.Add(argument);
        using var child = Process.Start(info) ?? throw new IOException("Commande d'installation indisponible.");
        var output = child.StandardOutput.ReadToEndAsync(); var error = child.StandardError.ReadToEndAsync();
        if (!child.WaitForExit(30000)) { child.Kill(true); throw new IOException("Délai d'installation dépassé."); }
        Task.WaitAll(output, error); return child.ExitCode;
    }
    private static void Protect(string path, bool publicRead)
    {
        if (Directory.Exists(path)) {
            if ((File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0) throw new IOException("Dossier lié refusé.");
            var owner = new DirectoryInfo(path).GetAccessControl().GetOwner(typeof(SecurityIdentifier))?.Value;
            if (owner is not ("S-1-5-18" or "S-1-5-32-544")) throw new IOException("Propriétaire du dossier existant inattendu.");
        }
        Directory.CreateDirectory(path);
        var acl = new DirectorySecurity(); acl.SetAccessRuleProtection(true, false);
        acl.SetOwner(new SecurityIdentifier("S-1-5-32-544"));
        foreach (var sid in new[] { "S-1-5-18", "S-1-5-32-544" }) acl.AddAccessRule(new FileSystemAccessRule(new SecurityIdentifier(sid), FileSystemRights.FullControl, InheritanceFlags.ContainerInherit | InheritanceFlags.ObjectInherit, PropagationFlags.None, AccessControlType.Allow));
        if (publicRead) acl.AddAccessRule(new FileSystemAccessRule(new SecurityIdentifier("S-1-5-32-545"), FileSystemRights.ReadAndExecute, InheritanceFlags.ContainerInherit | InheritanceFlags.ObjectInherit, PropagationFlags.None, AccessControlType.Allow));
        new DirectoryInfo(path).SetAccessControl(acl);
    }
    private static void Stop()
    {
        using var service = ServiceController.GetServices().FirstOrDefault(item => item.ServiceName == Service);
        if (service != null && service.Status != ServiceControllerStatus.Stopped) {
            if (service.Status != ServiceControllerStatus.StopPending) service.Stop();
            service.WaitForStatus(ServiceControllerStatus.Stopped, TimeSpan.FromSeconds(20));
        }
        foreach (var child in Process.GetProcessesByName("VaultAgent").Concat(Process.GetProcessesByName("VaultAgent.Provider"))) using (child) {
            try { if (new[] { Executable, Path.Combine(Root, "VaultAgent.Provider.exe") }.Contains(child.MainModule?.FileName, StringComparer.OrdinalIgnoreCase)) { child.Kill(true); child.WaitForExit(5000); } }
            catch (ArgumentException) { }
        }
    }
    public static void Install()
    {
        Payload.Verify();
        Protect(Root, true);
        Protect(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "HomelabVaultAgent"), false);
        using var backup = new InstallationBackup();
        try { Stop(); InstallFilesAndService(); }
        catch {
            if (backup.Exists) {
                Stop(); backup.Restore();
                using var previous = new ServiceController(Service); previous.Start();
                previous.WaitForStatus(ServiceControllerStatus.Running, TimeSpan.FromSeconds(20));
            }
            throw;
        }
    }
    private static void InstallFilesAndService()
    {
        Payload.Extract();
        if (Run(Path.Combine(Environment.SystemDirectory, "icacls.exe"), Root, "/setowner", "*S-1-5-32-544", "/T", "/Q") != 0) throw new IOException("Protection des fichiers échouée.");
        var sc = Path.Combine(Environment.SystemDirectory, "sc.exe");
        var exists = Run(sc, "query", Service) == 0;
        if (Run(sc, exists ? "config" : "create", Service, "binPath=", "\"" + Executable + "\" --service", "start=", "auto", "DisplayName=", "VaultAgent · Coffre local") != 0) throw new IOException("Création du service échouée.");
        Run(sc, "description", Service, "Déverrouillage Windows et remplissage local Bitwarden/Vaultwarden.");
        using (var service = new ServiceController(Service)) { service.Start(); service.WaitForStatus(ServiceControllerStatus.Running, TimeSpan.FromSeconds(20)); }
        using var registry = Registry.LocalMachine.CreateSubKey(UninstallKey);
        registry.SetValue("DisplayName", "VaultAgent"); registry.SetValue("DisplayVersion", typeof(SetupOperations).Assembly.GetName().Version!.ToString(3));
        registry.SetValue("Publisher", "MonsieurZed"); registry.SetValue("InstallLocation", Root);
        registry.SetValue("DisplayIcon", Executable); registry.SetValue("URLInfoAbout", "https://github.com/MonsieurZed/VaultAgent");
        var remove = "\"" + Path.Combine(Environment.SystemDirectory, @"WindowsPowerShell\v1.0\powershell.exe") + "\" -NoProfile -ExecutionPolicy Bypass -File \"" + Path.Combine(Root, @"setup\uninstall.ps1") + "\"";
        registry.SetValue("UninstallString", remove); registry.SetValue("QuietUninstallString", remove + " -Silent");
        registry.SetValue("NoModify", 1, RegistryValueKind.DWord); registry.SetValue("NoRepair", 1, RegistryValueKind.DWord);
        registry.SetValue("EstimatedSize", (int)(Directory.EnumerateFiles(Root, "*", SearchOption.AllDirectories).Sum(file => new FileInfo(file).Length) / 1024), RegistryValueKind.DWord);
        WriteResult(true, "Installation terminée.");
    }
    public static void Shortcuts(bool desktop, bool startup)
    {
        var directories = new List<string> { Environment.GetFolderPath(Environment.SpecialFolder.Programs) };
        if (desktop) directories.Add(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory));
        if (startup) directories.Add(Environment.GetFolderPath(Environment.SpecialFolder.Startup));
        dynamic shell = Activator.CreateInstance(Type.GetTypeFromProgID("WScript.Shell")!)!;
        foreach (var directory in directories) {
            Directory.CreateDirectory(directory); dynamic shortcut = shell.CreateShortcut(Path.Combine(directory, "Coffre Bitwarden.lnk"));
            shortcut.TargetPath = Executable; shortcut.Arguments = "--tray"; shortcut.WorkingDirectory = Root; shortcut.IconLocation = Executable + ",0"; shortcut.Save();
        }
    }
}
