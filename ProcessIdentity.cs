using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace Homelab.Vault;

internal static class ProcessIdentity
{
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] private static extern int GetPackageFullName(IntPtr process, ref uint length, StringBuilder? name);
    public static bool IsPasskeyProvider(int pid) {
        if (!IsInstalled(pid, "VaultAgent.Provider.exe")) return false;
        using var process = Process.GetProcessById(pid); uint length = 0;
        if (GetPackageFullName(process.Handle, ref length, null) != 122 || length > 4096) return false;
        var name = new StringBuilder((int)length);
        return GetPackageFullName(process.Handle, ref length, name) == 0 && name.ToString().StartsWith(Passkeys.WindowsPasskeyApi.PackageName + "_", StringComparison.Ordinal);
    }
    public static bool IsInstalled(int pid, string name)
    {
        using var process = Process.GetProcessById(pid);
        return string.Equals(process.MainModule?.FileName, Path.Combine(AppPaths.Install, name), StringComparison.OrdinalIgnoreCase);
    }
}
