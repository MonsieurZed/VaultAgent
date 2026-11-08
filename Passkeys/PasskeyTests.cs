using System.Buffers.Binary;
using System.Formats.Cbor;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;

namespace Homelab.Vault.Passkeys;

internal static class PasskeyTests
{
    [DllImport("webauthn.dll")] private static extern int WebAuthNDecodeMakeCredentialRequest(uint length, byte[] encoded, out IntPtr decoded);
    [DllImport("webauthn.dll")] private static extern void WebAuthNFreeDecodedMakeCredentialRequest(IntPtr decoded);
    private static void Check(bool valid, string name) { if (!valid) throw new Exception("Échec passkeys : " + name); }
    private static void Reject(Action action, string name) { try { action(); } catch { return; } throw new Exception("Accepté à tort : " + name); }
    public static byte[] Request(string rp = "example.test", bool algorithm = true)
    {
        var writer = new CborWriter(CborConformanceMode.Ctap2Canonical);
        writer.WriteStartMap(5); writer.WriteInt32(1); writer.WriteByteString(SHA256.HashData(Encoding.UTF8.GetBytes("challenge de test factice")));
        writer.WriteInt32(2); writer.WriteStartMap(2); writer.WriteTextString("id"); writer.WriteTextString(rp); writer.WriteTextString("name"); writer.WriteTextString("Site de test"); writer.WriteEndMap();
        writer.WriteInt32(3); writer.WriteStartMap(3); writer.WriteTextString("id"); writer.WriteByteString(new byte[] { 1, 2, 3, 4 }); writer.WriteTextString("name"); writer.WriteTextString("exemple@example.invalid"); writer.WriteTextString("displayName"); writer.WriteTextString("Compte factice"); writer.WriteEndMap();
        writer.WriteInt32(4); writer.WriteStartArray(1); writer.WriteStartMap(2); writer.WriteTextString("alg"); writer.WriteInt32(algorithm ? -7 : -257); writer.WriteTextString("type"); writer.WriteTextString("public-key"); writer.WriteEndMap(); writer.WriteEndArray();
        writer.WriteInt32(7); writer.WriteStartMap(2); writer.WriteTextString("rk"); writer.WriteBoolean(true); writer.WriteTextString("uv"); writer.WriteBoolean(true); writer.WriteEndMap(); writer.WriteEndMap(); return writer.Encode();
    }
    public static void Run()
    {
        var data = Request(); var request = CtapRequest.Read(data, true);
        Check(request.RpId == "example.test" && request.ClientHash.Length == 32 && request.UserId!.Length == 4 && request.RequireVerification, "lecture CTAP");
        Reject(() => CtapRequest.Read(Request("https://example.test"), true), "RP URL");
        Reject(() => CtapRequest.Read(Request(algorithm: false), true), "algorithme absent");
        Reject(() => CtapRequest.Read(data.Concat(new byte[] { 0 }).ToArray(), true), "octets supplémentaires");
        Reject(() => CtapRequest.Read(new byte[65537], true), "taille CTAP");
        if (WindowsPasskeyApi.Available) {
            var error = WebAuthNDecodeMakeCredentialRequest((uint)data.Length, data, out var decoded);
            Check(error == 0, "compatibilité du décodeur Windows " + error.ToString("X8"));
            WebAuthNFreeDecodedMakeCredentialRequest(decoded);
        }
        var credential = PasskeyCredential.Create(request);
        Check(credential.CredentialId.StartsWith("b64.") && credential.IdBytes().Length == 32, "identifiant compatible Bitwarden");
        Check(!credential.ToString().Contains(credential.KeyValue), "clé privée masquée en diagnostic");
        var reader = new CborReader(credential.Attestation(), CborConformanceMode.Ctap2Canonical);
        Check(reader.ReadStartMap() == 3 && reader.ReadInt32() == 1 && reader.ReadTextString() == "none" && reader.ReadInt32() == 2, "attestation none");
        var auth = reader.ReadByteString(); Check(auth[32] == 0x5D && BinaryPrimitives.ReadUInt16BigEndian(auth.AsSpan(53, 2)) == 32, "drapeaux et credential");
        Check(auth.AsSpan(0, 32).SequenceEqual(SHA256.HashData(Encoding.UTF8.GetBytes(request.RpId))), "liaison RP");
        Check(auth.AsSpan(55, 32).SequenceEqual(credential.IdBytes()), "identifiant attesté");
        var cose = new CborReader(auth.AsMemory(87), CborConformanceMode.Ctap2Canonical); Check(cose.ReadStartMap() == 5, "clé COSE");
        Check(cose.ReadInt32() == 1 && cose.ReadInt32() == 2 && cose.ReadInt32() == 3 && cose.ReadInt32() == -7 && cose.ReadInt32() == -1 && cose.ReadInt32() == 1 && cose.ReadInt32() == -2, "paramètres COSE");
        var x = cose.ReadByteString(); Check(cose.ReadInt32() == -3, "coordonnée COSE"); var y = cose.ReadByteString();
        var assertion = new CborReader(credential.Assertion(request), CborConformanceMode.Ctap2Canonical);
        Check(assertion.ReadStartMap() == 5 && assertion.ReadInt32() == 1, "assertion"); assertion.SkipValue();
        Check(assertion.ReadInt32() == 2, "authenticatorData"); var signedAuth = assertion.ReadByteString();
        Check(assertion.ReadInt32() == 3, "signature"); var signature = assertion.ReadByteString();
        using var verifier = ECDsa.Create(new ECParameters { Curve = ECCurve.NamedCurves.nistP256, Q = new ECPoint { X = x, Y = y } });
        Check(verifier.VerifyData(signedAuth.Concat(request.ClientHash).ToArray(), signature, HashAlgorithmName.SHA256, DSASignatureFormat.Rfc3279DerSequence), "signature WebAuthn indépendante");
        Reject(() => credential.Assertion(request with { RpId = "other.test" }), "autre RP");
        Reject(() => (credential with { Counter = "1" }).Assertion(request), "compteur non pris en charge");
        using var signer = new ECDsaCng(ECCurve.NamedCurves.nistP256);
        var publicKey = signer.Key.Export(CngKeyBlobFormat.EccPublicBlob); var hash = SHA256.HashData(data);
        var proof = signer.SignHash(hash, DSASignatureFormat.IeeeP1363FixedFieldConcatenation);
        PasskeySignature.Verify(publicKey, hash, proof); hash[0] ^= 1;
        Reject(() => PasskeySignature.Verify(publicKey, hash, proof), "preuve Windows altérée");
        using var rsa = new RSACng(2048); var rsaPublic = rsa.Key.Export(CngKeyBlobFormat.GenericPublicBlob);
        PasskeySignature.Verify(rsaPublic, hash, rsa.SignHash(hash, HashAlgorithmName.SHA256, RSASignaturePadding.Pss));
        PasskeySignature.Verify(rsaPublic, hash, rsa.SignHash(hash, HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1));
        Check(Marshal.SizeOf<PluginRequest>() == 56 && Marshal.SizeOf<PluginResponse>() == 16 && Marshal.SizeOf<PluginCancel>() == 32, "ABI COM x64");
        Check(Marshal.SizeOf<WindowsPasskeyApi.AddOptions>() == 80 && Marshal.SizeOf<WindowsPasskeyApi.VerificationRequest>() == 48, "ABI API Windows");
        var info = new CborReader(ProviderRegistration.GetInfo(), CborConformanceMode.Ctap2Canonical); info.SkipValue(); Check(info.BytesRemaining == 0, "GetInfo CTAP");
    }
}
