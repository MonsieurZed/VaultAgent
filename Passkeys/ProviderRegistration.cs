using System.Formats.Cbor;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;

namespace Homelab.Vault.Passkeys;

internal static class ProviderRegistration
{
    public static byte[] GetInfo()
    {
        var writer = new CborWriter(CborConformanceMode.Ctap2Canonical);
        writer.WriteStartMap(6); writer.WriteInt32(1); writer.WriteStartArray(2); writer.WriteTextString("FIDO_2_0"); writer.WriteTextString("FIDO_2_1"); writer.WriteEndArray();
        writer.WriteInt32(3); writer.WriteByteString(WindowsPasskeyApi.Aaguid.ToByteArray(true)); writer.WriteInt32(4);
        writer.WriteStartMap(4); writer.WriteTextString("rk"); writer.WriteBoolean(true); writer.WriteTextString("up"); writer.WriteBoolean(true); writer.WriteTextString("uv"); writer.WriteBoolean(true); writer.WriteTextString("alwaysUv"); writer.WriteBoolean(true); writer.WriteEndMap();
        writer.WriteInt32(5); writer.WriteInt32(65536); writer.WriteInt32(9); writer.WriteStartArray(1); writer.WriteTextString("internal"); writer.WriteEndArray();
        writer.WriteInt32(10); writer.WriteStartArray(1); writer.WriteStartMap(2); writer.WriteTextString("alg"); writer.WriteInt32(-7); writer.WriteTextString("type"); writer.WriteTextString("public-key"); writer.WriteEndMap(); writer.WriteEndArray(); writer.WriteEndMap(); return writer.Encode();
    }
    public static async Task RegisterAsync()
    {
        if (!WindowsPasskeyApi.Available || !WindowsPasskeyApi.HasOwnIdentity) throw new InvalidOperationException("Installer l'identité Windows du fournisseur avant de l'enregistrer.");
        if (!HelloNative.Available) throw new InvalidOperationException("Configurer Windows Hello dans les options de connexion Windows avant d'activer les passkeys.");
        using var memory = new NativeMemory(); var info = GetInfo();
        var logo = Convert.ToBase64String(Encoding.UTF8.GetBytes(File.ReadAllText(Path.Combine(AppPaths.Install, "assets", "shield.svg"))));
        var options = new WindowsPasskeyApi.AddOptions { Name = memory.Text("VaultAgent"), Clsid = memory.Structure(WindowsPasskeyApi.ClassId), RpId = memory.Text(WindowsPasskeyApi.RpId),
            LightLogo = memory.Text(logo), DarkLogo = memory.Text(logo), InfoLength = (uint)info.Length, Info = memory.Bytes(info) };
        if (!WindowsPasskeyApi.State().Registered) {
            WindowsPasskeyApi.Check(WindowsPasskeyApi.WebAuthNPluginAddAuthenticator2(options, out var result));
            WindowsPasskeyApi.WebAuthNPluginFreeAddAuthenticatorResponse(result);
        }
        await RefreshKeysAsync().ConfigureAwait(false);
    }
    private delegate int ReadKey(in Guid clsid, out uint size, out IntPtr key);
    private static byte[] Read(ReadKey read) {
        WindowsPasskeyApi.Check(read(WindowsPasskeyApi.ClassId, out var size, out var pointer));
        try { return WindowsPasskeyApi.Copy(pointer, size); } finally { WindowsPasskeyApi.WebAuthNPluginFreePublicKeyResponse(pointer); }
    }
    public static async Task RefreshKeysAsync() {
        if (!WindowsPasskeyApi.HasOwnIdentity) throw new UnauthorizedAccessException("Identité Windows du fournisseur requise.");
        var operation = Read(WindowsPasskeyApi.WebAuthNPluginGetOperationSigningPublicKey);
        byte[]? verification = null;
        try { verification = Read(WindowsPasskeyApi.WebAuthNPluginGetUserVerificationPublicKey); } catch (InvalidOperationException) { }
        await PipeTransport.SendAsync("passkey-register", new ProviderPublicKeys(operation, verification)).ConfigureAwait(false);
    }
    public static byte[] VerifyUser(IntPtr window, Guid transaction, string user, string rp, byte[] challenge)
    {
        using var memory = new NativeMemory();
        var request = new WindowsPasskeyApi.VerificationRequest { Window = window, Transaction = memory.Structure(transaction),
            Username = memory.Text(user), Hint = memory.Text(rp), Length = (uint)challenge.Length, Data = memory.Bytes(challenge) };
        WindowsPasskeyApi.Check(WindowsPasskeyApi.WebAuthNPluginPerformUserVerification2(request, out var size, out var pointer));
        try { return WindowsPasskeyApi.Copy(pointer, size); } finally { WindowsPasskeyApi.WebAuthNPluginFreeUserVerificationResponse(pointer); }
    }
}
