using System.Runtime.InteropServices;
using Microsoft.Win32;

namespace Homelab.Vault.Passkeys;

internal static class ProviderActivation
{
    public static bool IsInstalled { get { using var key = Registry.CurrentUser.OpenSubKey(@"Software\MonsieurZed\VaultAgent\Passkeys"); return key?.GetValue("PackageFamilyName") is string; } }
    [ComImport, Guid("45BA127D-10A8-46EA-8AB7-56EA9078943C")] private class ActivationManager { }
    [ComImport, Guid("2e941141-7f97-4756-ba1d-9decde894a3d"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IActivationManager {
        [PreserveSig] int ActivateApplication([MarshalAs(UnmanagedType.LPWStr)] string appId, [MarshalAs(UnmanagedType.LPWStr)] string arguments, uint options, out uint pid);
    }
    public static void Launch(string arguments)
    {
        using var settings = Registry.CurrentUser.OpenSubKey(@"Software\MonsieurZed\VaultAgent\Passkeys");
        var family = settings?.GetValue("PackageFamilyName") as string;
        if (family == null || !family.StartsWith(WindowsPasskeyApi.PackageName + "_", StringComparison.Ordinal) || family.Any(c => !char.IsAsciiLetterOrDigit(c) && c is not ('.' or '_'))) throw new InvalidOperationException("Installer l'identité Windows de VaultAgent avant cette opération.");
        var manager = new ActivationManager();
        try { WindowsPasskeyApi.Check(((IActivationManager)manager).ActivateApplication(family + "!VaultAgent", arguments, 0, out _)); }
        finally { Marshal.FinalReleaseComObject(manager); }
    }
    public static void WriteAssets(string directory)
    {
        Directory.CreateDirectory(directory);
        foreach (var pair in new[] { ("passkey-44.png", 44), ("passkey-150.png", 150), ("passkey-store.png", 50) }) {
            using var image = ShieldIcon.Render(pair.Item2); image.Save(Path.Combine(directory, pair.Item1));
        }
    }
}
