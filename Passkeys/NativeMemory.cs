using System.Runtime.InteropServices;

namespace Homelab.Vault.Passkeys;

internal sealed class NativeMemory : IDisposable
{
    private readonly List<IntPtr> buffers = new();
    public IntPtr Bytes(byte[] bytes) { var pointer = Marshal.AllocCoTaskMem(bytes.Length); Marshal.Copy(bytes, 0, pointer, bytes.Length); buffers.Add(pointer); return pointer; }
    public IntPtr Text(string text) { var pointer = Marshal.StringToCoTaskMemUni(text); buffers.Add(pointer); return pointer; }
    public IntPtr Structure<T>(T value) where T : struct { var pointer = Marshal.AllocCoTaskMem(Marshal.SizeOf<T>()); Marshal.StructureToPtr(value, pointer, false); buffers.Add(pointer); return pointer; }
    public void Dispose() { foreach (var pointer in buffers) Marshal.FreeCoTaskMem(pointer); }
}
