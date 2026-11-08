using System.Runtime.InteropServices;

namespace Homelab.Vault.Passkeys;

[StructLayout(LayoutKind.Sequential)]
public struct PluginRequest {
    public IntPtr Window; public Guid TransactionId; public uint SignatureLength; public IntPtr Signature;
    public uint RequestType, DataLength; public IntPtr Data;
}
[StructLayout(LayoutKind.Sequential)] public struct PluginResponse { public uint Length; public IntPtr Data; }
[StructLayout(LayoutKind.Sequential)] public struct PluginCancel { public Guid TransactionId; public uint SignatureLength; public IntPtr Signature; }

[ComVisible(true), Guid("d26bcf6f-b54c-43ff-9f06-d5bf148625f7"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IPluginAuthenticator {
    [PreserveSig] int MakeCredential(in PluginRequest request, out PluginResponse response);
    [PreserveSig] int GetAssertion(in PluginRequest request, out PluginResponse response);
    [PreserveSig] int CancelOperation(in PluginCancel request);
    [PreserveSig] int GetLockStatus(out uint status);
}
[ComVisible(true), Guid("00000001-0000-0000-C000-000000000046"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IPluginClassFactory {
    [PreserveSig] int CreateInstance(IntPtr outer, in Guid iid, out IntPtr instance);
    [PreserveSig] int LockServer([MarshalAs(UnmanagedType.Bool)] bool value);
}
