using System.Diagnostics;

namespace Homelab.Vault;

internal static class ProcessIdentity
{
    public static bool IsInstalled(int pid, string name)
    {
        using var process = Process.GetProcessById(pid);
        return string.Equals(process.MainModule?.FileName, Path.Combine(AppPaths.Install, name), StringComparison.OrdinalIgnoreCase);
    }
}
