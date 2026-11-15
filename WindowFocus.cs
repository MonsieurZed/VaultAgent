// Homelab VaultAgent · mise au premier plan d'une fenêtre identifiée par processus et titre.
// Aucune frappe n'est envoyée : seule la fenêtre exactement identifiée peut être activée.

using System.Runtime.InteropServices;

namespace Homelab.Vault;

internal static class WindowFocus
{
    private delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);
    private const int RestoreCommand = 9;
    private const uint MouseMove = 0x0001;

    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] private static extern bool IsIconic(IntPtr window);
    [DllImport("user32.dll")] private static extern bool ShowWindow(IntPtr window, int command);
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] private static extern bool BringWindowToTop(IntPtr window);
    [DllImport("user32.dll")] private static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
    [DllImport("user32.dll")] private static extern bool AttachThreadInput(uint attach, uint to, bool value);
    [DllImport("user32.dll")] private static extern void mouse_event(uint flags, int x, int y, uint data, UIntPtr extra);
    [DllImport("kernel32.dll")] private static extern uint GetCurrentThreadId();

    /// <summary>Liste les fenêtres visibles qui satisfont les exigences.</summary>
    /// <param name="requirement">Processus et fragment de titre exigés.</param>
    /// <returns>Fenêtres correspondantes.</returns>
    public static IReadOnlyList<WindowTarget> Find(TargetRequirement requirement)
    {
        var matches = new List<WindowTarget>();
        EnumWindows((window, _) => {
            if (!IsWindowVisible(window)) return true;
            var target = WindowTarget.FromHandle(window);
            if (target is not null && requirement.Matches(target.ProcessName, target.Title)) matches.Add(target);
            return true;
        }, IntPtr.Zero);
        return matches;
    }

    /// <summary>Met au premier plan l'unique fenêtre qui satisfait les exigences et vérifie le résultat.</summary>
    /// <param name="requirement">Processus et fragment de titre exigés.</param>
    /// <returns>Fenêtre désormais au premier plan.</returns>
    /// <exception cref="TargetMismatchException">Aucune fenêtre, plusieurs fenêtres, ou activation refusée par Windows.</exception>
    public static WindowTarget Focus(TargetRequirement requirement)
    {
        var matches = Find(requirement);
        if (matches.Count == 0) throw new TargetMismatchException("Aucune fenêtre ne correspond au processus et au titre exigés.", null, requirement);
        if (matches.Count > 1) throw new TargetMismatchException("Plusieurs fenêtres correspondent ; préciser --title.", null, requirement);
        var target = matches[0];
        if (IsIconic(target.Handle)) ShowWindow(target.Handle, RestoreCommand);
        Raise(target.Handle);
        var after = WindowTarget.Foreground();
        if (!after.IsSameWindow(target) || !requirement.Matches(after.ProcessName, after.Title))
            throw new TargetMismatchException("Windows a refusé de mettre la fenêtre au premier plan.", after, requirement);
        return after;
    }

    private static void Raise(IntPtr window)
    {
        mouse_event(MouseMove, 0, 0, 0, UIntPtr.Zero);
        var foregroundThread = GetWindowThreadProcessId(GetForegroundWindow(), out _);
        var current = GetCurrentThreadId();
        var attached = foregroundThread != current && AttachThreadInput(current, foregroundThread, true);
        try { SetForegroundWindow(window); BringWindowToTop(window); }
        finally { if (attached) AttachThreadInput(current, foregroundThread, false); }
        Thread.Sleep(150);
    }
}
