using System.Runtime.InteropServices;

namespace Homelab.Vault.Passkeys;

internal static class ProviderHost
{
    [DllImport("ole32.dll")] private static extern int CoRegisterClassObject(in Guid clsid, IntPtr instance, uint context, uint flags, out uint cookie);
    [DllImport("ole32.dll")] private static extern int CoRevokeClassObject(uint cookie);
    public static void Run()
    {
        if (!WindowsPasskeyApi.HasOwnIdentity) throw new InvalidOperationException("Le fournisseur doit être activé par son identité Windows enregistrée.");
        using var dispatcher = new Control(); dispatcher.CreateControl(); _ = dispatcher.Handle;
        var authenticator = new PluginAuthenticator(dispatcher); var factory = new PluginClassFactory(authenticator);
        var pointer = Marshal.GetIUnknownForObject(factory);
        try {
            WindowsPasskeyApi.Check(CoRegisterClassObject(WindowsPasskeyApi.ClassId, pointer, 4, 1, out var cookie));
            try { Application.Run(); } finally { CoRevokeClassObject(cookie); }
        }
        finally { Marshal.Release(pointer); GC.KeepAlive(factory); }
    }
}

[ComVisible(true), ClassInterface(ClassInterfaceType.None)]
public sealed class PluginClassFactory : IPluginClassFactory
{
    private readonly PluginAuthenticator authenticator;
    internal PluginClassFactory(PluginAuthenticator authenticator) { this.authenticator = authenticator; }
    public int CreateInstance(IntPtr outer, in Guid iid, out IntPtr instance) {
        instance = IntPtr.Zero; if (outer != IntPtr.Zero) return unchecked((int)0x80040110);
        var pointer = Marshal.GetIUnknownForObject(authenticator);
        try { var requested = iid; return Marshal.QueryInterface(pointer, in requested, out instance); }
        finally { Marshal.Release(pointer); }
    }
    public int LockServer(bool value) => 0;
}
