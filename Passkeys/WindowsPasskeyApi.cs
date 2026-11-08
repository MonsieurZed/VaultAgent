using System.Runtime.InteropServices;
using System.Text;

[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
namespace Homelab.Vault.Passkeys;
internal static class WindowsPasskeyApi
{
    public static readonly Guid ClassId = new("58f03e8c-5d54-4d39-b9c3-47aa36d3a1f6");
    public static readonly Guid Aaguid = new("6ea77391-fd95-4144-98dd-4e3e622aa035");
    public const string RpId = "passkeys.vaultagent.local";
    public const string PackageName = "MonsieurZed.VaultAgent.Passkeys";
    [StructLayout(LayoutKind.Sequential)] internal struct AddOptions {
        public IntPtr Name, Clsid, RpId, LightLogo, DarkLogo; public uint InfoLength; public IntPtr Info;
        public uint RpCount; public IntPtr Rps, UserVerificationKeyName;
    }
    [StructLayout(LayoutKind.Sequential)] internal struct Buffer { public uint Length; public IntPtr Data; }
    [StructLayout(LayoutKind.Sequential)] internal struct VerificationRequest {
        public IntPtr Window, Transaction, Username, Hint; public uint Length; public IntPtr Data;
    }
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] private static extern int GetCurrentPackageFullName(ref uint length, StringBuilder? name);
    [DllImport("webauthn.dll")] internal static extern int WebAuthNPluginAddAuthenticator2(in AddOptions options, out IntPtr response);
    [DllImport("webauthn.dll")] internal static extern int WebAuthNPluginRemoveAuthenticator(in Guid clsid);
    [DllImport("webauthn.dll")] private static extern int WebAuthNPluginGetAuthenticatorState(in Guid clsid, out uint state);
    [DllImport("webauthn.dll")] internal static extern void WebAuthNPluginFreeAddAuthenticatorResponse(IntPtr response);
    [DllImport("webauthn.dll")] internal static extern int WebAuthNPluginGetOperationSigningPublicKey(in Guid clsid, out uint size, out IntPtr key);
    [DllImport("webauthn.dll")] internal static extern int WebAuthNPluginGetUserVerificationPublicKey(in Guid clsid, out uint size, out IntPtr key);
    [DllImport("webauthn.dll")] internal static extern void WebAuthNPluginFreePublicKeyResponse(IntPtr key);
    [DllImport("webauthn.dll")] internal static extern int WebAuthNPluginPerformUserVerification2(in VerificationRequest request, out uint size, out IntPtr response);
    [DllImport("webauthn.dll")] internal static extern void WebAuthNPluginFreeUserVerificationResponse(IntPtr response);
    public static bool Available {
        get {
            if (!OperatingSystem.IsWindowsVersionAtLeast(10, 0, 26100)) return false;
            if (!NativeLibrary.TryLoad(Path.Combine(Environment.SystemDirectory, "webauthn.dll"), out var module)) return false;
            try { return new[] { "WebAuthNPluginAddAuthenticator2", "WebAuthNPluginPerformUserVerification2", "WebAuthNPluginGetOperationSigningPublicKey", "WebAuthNPluginGetUserVerificationPublicKey" }.All(name => NativeLibrary.TryGetExport(module, name, out _)); }
            finally { NativeLibrary.Free(module); }
        }
    }
    public static bool HasOwnIdentity {
        get {
            uint length = 0; if (GetCurrentPackageFullName(ref length, null) != 122 || length > 4096) return false;
            var name = new StringBuilder((int)length);
            return GetCurrentPackageFullName(ref length, name) == 0 && name.ToString().StartsWith(PackageName + "_", StringComparison.Ordinal);
        }
    }
    public static (bool Registered, bool Enabled, int Error) State() {
        if (!Available) return (false, false, unchecked((int)0x80004001));
        var error = WebAuthNPluginGetAuthenticatorState(ClassId, out var state); return (error == 0, error == 0 && state == 1, error);
    }
    public static object Status() {
        var state = State();
        return new { apiAvailable = Available, ownPackageIdentity = HasOwnIdentity, registered = state.Registered, enabled = state.Enabled,
            windowsHelloAvailable = HelloNative.Available, remoteSession = SystemInformation.TerminalServerSession,
            stateError = state.Error.ToString("X8"), providerImplemented = true, experimental = true, endToEndValidated = false };
    }
    public static byte[] Copy(IntPtr data, uint size) {
        if (size is 0 or > 1024 * 1024 || data == IntPtr.Zero) throw new InvalidOperationException("Réponse du fournisseur Windows invalide.");
        var result = new byte[size]; Marshal.Copy(data, result, 0, result.Length); return result;
    }
    public static void Check(int error) { if (error != 0) throw new InvalidOperationException("Opération passkey Windows refusée (0x" + error.ToString("X8") + ")."); }
}
