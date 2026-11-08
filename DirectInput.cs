using System.ComponentModel;
using System.Runtime.InteropServices;

namespace Homelab.Vault;

internal static class DirectInput
{
    [StructLayout(LayoutKind.Sequential)] private struct Keyboard { public ushort VirtualKey, Scan; public uint Flags, Time; public UIntPtr Extra; }
    [StructLayout(LayoutKind.Explicit, Size = 40)] private struct Input { [FieldOffset(0)] public uint Type; [FieldOffset(8)] public Keyboard Key; }
    [DllImport("user32.dll")] internal static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll", SetLastError = true)] private static extern uint SendInput(uint count, Input[] input, int size);
    public static void Type(string text, IntPtr expectedWindow)
    {
        if (expectedWindow == IntPtr.Zero || GetForegroundWindow() != expectedWindow) throw new InvalidOperationException("Le champ cible a perdu le focus. Recliquer dans le champ et réessayer.");
        foreach (var chunk in text.Chunk(128)) {
            if (GetForegroundWindow() != expectedWindow) throw new InvalidOperationException("La fenêtre cible a changé ; saisie interrompue.");
            var events = chunk.SelectMany(character => new[] {
                new Input { Type = 1, Key = new Keyboard { Scan = character, Flags = 4 } },
                new Input { Type = 1, Key = new Keyboard { Scan = character, Flags = 6 } }
            }).ToArray();
            if (SendInput((uint)events.Length, events, Marshal.SizeOf<Input>()) != events.Length) throw new Win32Exception(Marshal.GetLastWin32Error(), "Saisie directe refusée par Windows.");
        }
    }
}
