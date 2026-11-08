using System.Buffers.Binary;
using System.Formats.Cbor;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Homelab.Vault.Passkeys;

internal sealed record PasskeyCredential(string CredentialId, string KeyType, string KeyAlgorithm, string KeyCurve, string KeyValue,
    string RpId, string UserHandle, string UserName, string UserDisplayName, string RpName, string Counter, string Discoverable, DateTimeOffset CreationDate)
{
    public override string ToString() => "PasskeyCredential { RpId = " + RpId + ", KeyValue = <redacted> }";
    public static PasskeyCredential Create(PasskeyRequest request) {
        using var key = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        var privateKey = key.ExportPkcs8PrivateKey();
        try { return new("b64." + Encode(RandomNumberGenerator.GetBytes(32)), "public-key", "ECDSA", "P-256", Encode(privateKey),
            request.RpId, Encode(request.UserId!), request.UserName ?? "", request.DisplayName ?? request.UserName ?? "", request.RpName, "0", "true", DateTimeOffset.UtcNow); }
        finally { CryptographicOperations.ZeroMemory(privateKey); }
    }
    public byte[] IdBytes() {
        if (Guid.TryParse(CredentialId, out var id)) return id.ToByteArray(true);
        var encoded = CredentialId.StartsWith("b64.", StringComparison.Ordinal) ? CredentialId[4..] : CredentialId;
        return Decode(encoded);
    }
    public ECDsa OpenKey() {
        if (KeyType != "public-key" || KeyAlgorithm != "ECDSA" || KeyCurve != "P-256") throw new InvalidOperationException("Algorithme de passkey non pris en charge.");
        var bytes = Decode(KeyValue); var key = ECDsa.Create();
        try { key.ImportPkcs8PrivateKey(bytes, out var read); if (read != bytes.Length || key.KeySize != 256) throw new InvalidOperationException("Clé de passkey invalide."); return key; }
        catch { key.Dispose(); throw; } finally { CryptographicOperations.ZeroMemory(bytes); }
    }
    public byte[] AuthData(bool creation) {
        if (Counter != "0") throw new InvalidOperationException("Les passkeys avec compteur non nul ne sont pas prises en charge.");
        var data = new List<byte>(SHA256.HashData(Encoding.UTF8.GetBytes(RpId))) { (byte)(0x01 | 0x04 | 0x08 | 0x10 | (creation ? 0x40 : 0)) };
        data.AddRange(new byte[4]);
        if (creation) {
            data.AddRange(WindowsPasskeyApi.Aaguid.ToByteArray(true)); var id = IdBytes(); var length = new byte[2]; BinaryPrimitives.WriteUInt16BigEndian(length, checked((ushort)id.Length));
            data.AddRange(length); data.AddRange(id);
            using var key = OpenKey(); var point = key.ExportParameters(false); var cose = new CborWriter(CborConformanceMode.Ctap2Canonical);
            cose.WriteStartMap(5); cose.WriteInt32(1); cose.WriteInt32(2); cose.WriteInt32(3); cose.WriteInt32(-7); cose.WriteInt32(-1); cose.WriteInt32(1);
            cose.WriteInt32(-2); cose.WriteByteString(point.Q.X!); cose.WriteInt32(-3); cose.WriteByteString(point.Q.Y!); cose.WriteEndMap(); data.AddRange(cose.Encode());
        }
        return data.ToArray();
    }
    public byte[] Attestation() {
        var writer = new CborWriter(CborConformanceMode.Ctap2Canonical);
        writer.WriteStartMap(3); writer.WriteInt32(1); writer.WriteTextString("none"); writer.WriteInt32(2); writer.WriteByteString(AuthData(true));
        writer.WriteInt32(3); writer.WriteStartMap(0); writer.WriteEndMap(); writer.WriteEndMap(); return writer.Encode();
    }
    public byte[] Assertion(PasskeyRequest request) {
        if (!string.Equals(request.RpId, RpId, StringComparison.Ordinal)) throw new UnauthorizedAccessException("RP différent de la passkey.");
        var auth = AuthData(false); using var key = OpenKey();
        var signature = key.SignData(auth.Concat(request.ClientHash).ToArray(), HashAlgorithmName.SHA256, DSASignatureFormat.Rfc3279DerSequence);
        var writer = new CborWriter(CborConformanceMode.Ctap2Canonical);
        writer.WriteStartMap(5); writer.WriteInt32(1); writer.WriteStartMap(2); writer.WriteTextString("id"); writer.WriteByteString(IdBytes()); writer.WriteTextString("type"); writer.WriteTextString("public-key"); writer.WriteEndMap();
        writer.WriteInt32(2); writer.WriteByteString(auth); writer.WriteInt32(3); writer.WriteByteString(signature); writer.WriteInt32(4);
        writer.WriteStartMap(3); writer.WriteTextString("id"); writer.WriteByteString(Decode(UserHandle)); writer.WriteTextString("name"); writer.WriteTextString(UserName); writer.WriteTextString("displayName"); writer.WriteTextString(UserDisplayName); writer.WriteEndMap();
        writer.WriteInt32(5); writer.WriteInt32(1); writer.WriteEndMap(); return writer.Encode();
    }
    public static string Encode(byte[] bytes) => Convert.ToBase64String(bytes).TrimEnd('=').Replace('+', '-').Replace('/', '_');
    public static byte[] Decode(string text) => Convert.FromBase64String(text.Replace('-', '+').Replace('_', '/').PadRight((text.Length + 3) / 4 * 4, '='));
}
