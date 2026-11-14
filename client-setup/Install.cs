using System.Diagnostics;
using Microsoft.Win32;

namespace Homelab.Vault.ClientSetup;
internal static class Install
{
    public static string Root => Path.GetFullPath(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs", "VaultAgentClient"));
    public static string Exe => Path.Combine(Root, "VaultAgent-Client.exe");
    public static void Run(bool desktop, bool startup) {
        Package.Verify();
        if (Directory.Exists(Root) && (File.GetAttributes(Root) & FileAttributes.ReparsePoint) != 0) throw new IOException("Dossier lie refuse.");
        foreach (var process in Process.GetProcessesByName("VaultAgent-Client")) using (process) {
            try { if (string.Equals(process.MainModule?.FileName, Exe, StringComparison.OrdinalIgnoreCase)) { process.Kill(); process.WaitForExit(5000); } } catch (ArgumentException) { }
        }
        Directory.CreateDirectory(Root); Package.Extract();
        using (var run = Registry.CurrentUser.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Run")) {
            if (startup) run.SetValue("VaultAgentClient", "\"" + Exe + "\" tray");
            else if (run.GetValue("VaultAgentClient") is string value && value.Contains(Exe, StringComparison.OrdinalIgnoreCase)) run.DeleteValue("VaultAgentClient", false);
        }
        using (var key = Registry.CurrentUser.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\VaultAgentClient")) {
            key.SetValue("DisplayName", "VaultAgent Client"); key.SetValue("DisplayVersion", "3.0.0-preview.6"); key.SetValue("Publisher", "MonsieurZed"); key.SetValue("InstallLocation", Root); key.SetValue("DisplayIcon", Exe);
            var script = Path.Combine(Root, "uninstall.ps1");
            key.SetValue("UninstallString", "\"" + Path.Combine(Environment.SystemDirectory, @"WindowsPowerShell\v1.0\powershell.exe") + "\" -NoProfile -ExecutionPolicy Bypass -File \"" + script + "\"");
            key.SetValue("NoModify", 1); key.SetValue("NoRepair", 1);
        }
        dynamic shell = Activator.CreateInstance(Type.GetTypeFromProgID("WScript.Shell")!)!;
        foreach (var folder in new[] { Environment.GetFolderPath(Environment.SpecialFolder.Programs) }.Concat(desktop ? new[] { Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory) } : Array.Empty<string>())) {
            Directory.CreateDirectory(folder); dynamic link = shell.CreateShortcut(Path.Combine(folder, "VaultAgent Client.lnk")); link.TargetPath = Exe; link.Arguments = "tray"; link.WorkingDirectory = Root; link.Save();
        }
    }
}
