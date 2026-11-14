// Homelab VaultAgent · saisie clavier Unicode vers une fenêtre dont l'identité est vérifiée.

using System.ComponentModel;
using System.Runtime.InteropServices;

namespace Homelab.Vault;

internal static class DirectInput
{
    private const int ChunkSize = 32;
    [StructLayout(LayoutKind.Sequential)] private struct Keyboard { public ushort VirtualKey, Scan; public uint Flags, Time; public UIntPtr Extra; }
    [StructLayout(LayoutKind.Explicit, Size = 40)] private struct Input { [FieldOffset(0)] public uint Type; [FieldOffset(8)] public Keyboard Key; }
    [DllImport("user32.dll", SetLastError = true)] private static extern uint SendInput(uint count, Input[] input, int size);

    /// <summary>
    /// Vérifie que la fenêtre active est exactement celle capturée et qu'elle satisfait les exigences d'identité.
    /// </summary>
    /// <param name="expected">Fenêtre capturée au lancement de la commande.</param>
    /// <param name="requirement">Processus et fragment de titre exigés.</param>
    /// <exception cref="TargetMismatchException">La fenêtre active a changé ou ne correspond pas.</exception>
    public static void Verify(WindowTarget expected, TargetRequirement requirement)
    {
        WindowTarget current;
        try { current = WindowTarget.Foreground(); }
        catch (InvalidOperationException error) { throw new TargetMismatchException(error.Message, null, requirement); }
        if (!current.IsSameWindow(expected)) throw new TargetMismatchException("La fenêtre active a changé ; recliquer dans le champ cible et réessayer.", current, requirement);
        if (!requirement.Matches(current.ProcessName, current.Title)) throw new TargetMismatchException("La fenêtre active ne correspond pas au processus et au titre exigés ; saisie refusée.", current, requirement);
    }

    /// <summary>Tape le texte par blocs, en revérifiant l'identité de la fenêtre avant chaque bloc.</summary>
    /// <param name="text">Texte à saisir ; jamais copié dans le presse-papiers.</param>
    /// <param name="expected">Fenêtre capturée au lancement de la commande.</param>
    /// <param name="requirement">Processus et fragment de titre exigés.</param>
    /// <exception cref="TargetMismatchException">La fenêtre n'est plus la cible validée.</exception>
    public static void Type(string text, WindowTarget expected, TargetRequirement requirement)
    {
        foreach (var chunk in text.Chunk(ChunkSize)) {
            Verify(expected, requirement);
            var events = chunk.SelectMany(character => new[] {
                new Input { Type = 1, Key = new Keyboard { Scan = character, Flags = 4 } },
                new Input { Type = 1, Key = new Keyboard { Scan = character, Flags = 6 } }
            }).ToArray();
            if (SendInput((uint)events.Length, events, Marshal.SizeOf<Input>()) != events.Length) throw new Win32Exception(Marshal.GetLastWin32Error(), "Saisie directe refusée par Windows.");
        }
    }
}
