using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Homelab.Vault;

internal static class HelloNative
{
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] private struct Rp { public uint Version; public string Id, Name; public IntPtr Icon; }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] private struct User { public uint Version, Length; public IntPtr Id; public string Name; public IntPtr Icon; public string DisplayName; }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] private struct Algorithm { public uint Version; public string Type; public int Value; }
    [StructLayout(LayoutKind.Sequential)] private struct List { public uint Count; public IntPtr Values; }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] private struct ClientData { public uint Version, Length; public IntPtr Json; public string HashAlgorithm; }
    [StructLayout(LayoutKind.Sequential)] private struct MakeOptions { public uint Version, Timeout; public List Exclude, Extensions; public uint Attachment; public int Resident; public uint Verification, Attestation, Flags; }
    [StructLayout(LayoutKind.Sequential)] private struct GetOptions { public uint Version, Timeout; public List Allow, Extensions; public uint Attachment, Verification, Flags; }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] private struct Credential { public uint Version, Length; public IntPtr Id; public string Type; }
    [StructLayout(LayoutKind.Sequential)] private struct Attestation {
        public uint Version; public IntPtr Format; public uint AuthLength; public IntPtr AuthData;
        public uint AttestationLength; public IntPtr AttestationData; public uint DecodeType; public IntPtr Decode;
        public uint ObjectLength; public IntPtr Object; public uint IdLength; public IntPtr Id;
    }
    [StructLayout(LayoutKind.Sequential)] private struct Assertion {
        public uint Version, AuthLength; public IntPtr AuthData; public uint SignatureLength; public IntPtr Signature; public Credential Credential;
    }
    [DllImport("webauthn.dll")] private static extern int WebAuthNIsUserVerifyingPlatformAuthenticatorAvailable([MarshalAs(UnmanagedType.Bool)] out bool available);
    [DllImport("webauthn.dll")] private static extern uint WebAuthNGetApiVersionNumber();
    [DllImport("webauthn.dll", CharSet = CharSet.Unicode)] private static extern int WebAuthNAuthenticatorMakeCredential(IntPtr window, ref Rp rp, ref User user, ref List algorithms, ref ClientData data, ref MakeOptions options, out IntPtr result);
    [DllImport("webauthn.dll", CharSet = CharSet.Unicode)] private static extern int WebAuthNAuthenticatorGetAssertion(IntPtr window, string rp, ref ClientData data, ref GetOptions options, out IntPtr result);
    [DllImport("webauthn.dll")] private static extern void WebAuthNFreeCredentialAttestation(IntPtr result);
    [DllImport("webauthn.dll")] private static extern void WebAuthNFreeAssertion(IntPtr result);

    public static bool Available { get { try { return WebAuthNIsUserVerifyingPlatformAuthenticatorAvailable(out var available) == 0 && available; } catch { return false; } } }
    public static uint ApiVersion { get { try { return WebAuthNGetApiVersionNumber(); } catch { return 0; } } }
    private static string ClientJson(string type, string challenge) => JsonSerializer.Serialize(new { type, challenge, origin = AppPaths.Origin, crossOrigin = false });
    private static byte[] Copy(IntPtr address, uint length) {
        if (length > 1024 * 1024) throw new InvalidOperationException("Réponse Windows Hello invalide.");
        var bytes = new byte[length]; Marshal.Copy(address, bytes, 0, bytes.Length); return bytes;
    }
    private static void Check(int result) { if (result != 0) throw new InvalidOperationException("Windows Hello annulé ou indisponible (0x" + result.ToString("X8") + ")."); }
    public static HelloCredential Register(IntPtr window, string sid, string email)
    {
        using var memory = new Arena();
        var id = SHA256.HashData(Encoding.UTF8.GetBytes(sid));
        var rp = new Rp { Version = 1, Id = AppPaths.RpId, Name = "Coffre Bitwarden" };
        var user = new User { Version = 1, Length = (uint)id.Length, Id = memory.Bytes(id), Name = email, DisplayName = email };
        var parameters = memory.Algorithms(new[] { new Algorithm { Version = 1, Type = "public-key", Value = -7 }, new Algorithm { Version = 1, Type = "public-key", Value = -257 } });
        var algorithms = new List { Count = 2, Values = parameters };
        var json = Encoding.UTF8.GetBytes(ClientJson("webauthn.create", HelloVerification.Base64Url(RandomNumberGenerator.GetBytes(32))));
        var client = new ClientData { Version = 1, Length = (uint)json.Length, Json = memory.Bytes(json), HashAlgorithm = "SHA-256" };
        var options = new MakeOptions { Version = 1, Timeout = 60000, Attachment = 1, Verification = 1, Attestation = 1 };
        Check(WebAuthNAuthenticatorMakeCredential(window, ref rp, ref user, ref algorithms, ref client, ref options, out var result));
        try {
            var value = Marshal.PtrToStructure<Attestation>(result);
            return HelloVerification.ReadRegistration(Copy(value.Id, value.IdLength), Copy(value.AuthData, value.AuthLength));
        }
        finally { WebAuthNFreeCredentialAttestation(result); }
    }
    public static HelloProof Sign(IntPtr window, HelloCredential registered, string challenge)
    {
        using var memory = new Arena();
        var id = Convert.FromBase64String(registered.Id);
        var credential = new Credential { Version = 1, Length = (uint)id.Length, Id = memory.Bytes(id), Type = "public-key" };
        var json = Encoding.UTF8.GetBytes(ClientJson("webauthn.get", challenge));
        var client = new ClientData { Version = 1, Length = (uint)json.Length, Json = memory.Bytes(json), HashAlgorithm = "SHA-256" };
        var options = new GetOptions { Version = 1, Timeout = 60000, Attachment = 1, Verification = 1,
            Allow = new List { Count = 1, Values = memory.Structure(credential) } };
        Check(WebAuthNAuthenticatorGetAssertion(window, AppPaths.RpId, ref client, ref options, out var result));
        try {
            var value = Marshal.PtrToStructure<Assertion>(result);
            return new(Convert.ToBase64String(Copy(value.Credential.Id, value.Credential.Length)), Convert.ToBase64String(json),
                Convert.ToBase64String(Copy(value.AuthData, value.AuthLength)), Convert.ToBase64String(Copy(value.Signature, value.SignatureLength)));
        }
        finally { WebAuthNFreeAssertion(result); }
    }
    private sealed class Arena : IDisposable {
        private readonly System.Collections.Generic.List<Action> release = new();
        public IntPtr Bytes(byte[] bytes) { var p = Marshal.AllocHGlobal(bytes.Length); Marshal.Copy(bytes, 0, p, bytes.Length); release.Add(() => Marshal.FreeHGlobal(p)); return p; }
        public IntPtr Structure<T>(T value) where T : struct { var p = Marshal.AllocHGlobal(Marshal.SizeOf<T>()); Marshal.StructureToPtr(value, p, false); release.Add(() => { Marshal.DestroyStructure<T>(p); Marshal.FreeHGlobal(p); }); return p; }
        public IntPtr Algorithms(Algorithm[] values) {
            var size = Marshal.SizeOf<Algorithm>(); var p = Marshal.AllocHGlobal(size * values.Length);
            for (var i = 0; i < values.Length; i++) Marshal.StructureToPtr(values[i], p + i * size, false);
            release.Add(() => { for (var i = 0; i < values.Length; i++) Marshal.DestroyStructure<Algorithm>(p + i * size); Marshal.FreeHGlobal(p); }); return p;
        }
        public void Dispose() { foreach (var action in Enumerable.Reverse(release)) action(); }
    }
}
