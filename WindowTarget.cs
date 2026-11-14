// Homelab VaultAgent · lecture de l'identité de la fenêtre au premier plan.

using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace Homelab.Vault;

/// <summary>Instantané de la fenêtre au premier plan : handle, processus et titre.</summary>
internal sealed record WindowTarget(IntPtr Handle, int ProcessId, string ProcessName, string Title)
{
    [DllImport("user32.dll")] private static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowTextLengthW(IntPtr window);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowTextW(IntPtr window, StringBuilder text, int capacity);

    /// <summary>Lit la fenêtre actuellement au premier plan.</summary>
    /// <returns>Instantané de la fenêtre.</returns>
    /// <exception cref="InvalidOperationException">Aucune fenêtre active ou processus illisible.</exception>
    public static WindowTarget Foreground()
    {
        var handle = GetForegroundWindow();
        if (handle == IntPtr.Zero) throw new InvalidOperationException("Aucune fenêtre au premier plan.");
        if (GetWindowThreadProcessId(handle, out var pid) == 0) throw new InvalidOperationException("Processus de la fenêtre active introuvable.");
        string name;
        try { using var process = Process.GetProcessById((int)pid); name = process.ProcessName; }
        catch (Exception error) when (error is ArgumentException or InvalidOperationException or System.ComponentModel.Win32Exception) {
            throw new InvalidOperationException("Processus de la fenêtre active illisible.");
        }
        var builder = new StringBuilder(Math.Max(GetWindowTextLengthW(handle), 0) + 1);
        GetWindowTextW(handle, builder, builder.Capacity);
        return new WindowTarget(handle, (int)pid, name, builder.ToString());
    }

    /// <summary>Indique si deux instantanés désignent la même fenêtre du même processus.</summary>
    /// <param name="other">Instantané à comparer.</param>
    /// <returns>Vrai si handle et processus sont identiques.</returns>
    public bool IsSameWindow(WindowTarget other) => Handle == other.Handle && ProcessId == other.ProcessId;

    /// <summary>Représentation sérialisable sans handle.</summary>
    public object Describe() => new { process = ProcessName, title = Title };
}

/// <summary>Refus de saisie : la fenêtre active n'est pas celle exigée ou a changé.</summary>
internal sealed class TargetMismatchException(string reason, WindowTarget? foreground, TargetRequirement requirement) : InvalidOperationException(reason)
{
    public WindowTarget? Foreground { get; } = foreground;
    public TargetRequirement Requirement { get; } = requirement;
}
