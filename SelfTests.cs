using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Homelab.Vault;

internal static class SelfTests
{
    private static void Check(bool condition, string name) { if (!condition) throw new Exception("Échec : " + name); }
    private static void Reject(Action action, string name) { try { action(); } catch { return; } throw new Exception("Accepté à tort : " + name); }
    public static void Run()
    {
        long ticks = 0; var lease = new SlidingLease(() => ticks);
        lease.Open(15); Check(lease.IsValid, "ouverture");
        ticks += 14 * 60L * Stopwatch.Frequency; lease.Touch();
        ticks += 14 * 60L * Stopwatch.Frequency; Check(lease.IsValid, "prolongation");
        ticks += 2 * 60L * Stopwatch.Frequency; Check(!lease.IsValid, "expiration");
        lease.Open(null); ticks += 1000000L * Stopwatch.Frequency; Check(lease.IsValid && lease.ExpiresUtc == null, "délai désactivé");
        lease.Close(); Check(!lease.IsValid, "verrouillage manuel");
        Check(CliRequest.Parse(new[] { "list", "items", "--search", "test" }).Path == "/list/object/items?search=test", "liste");
        Check(CliRequest.Parse(new[] { "get", "password", "a b" }).Path == "/object/password/a%20b", "encodage");
        foreach (var command in new[] { new[] { "serve" }, new[] { "get", "attachment", "id" }, new[] { "get", "item", "id", "--output", "file" }, new[] { "sync", "--session", "key" } }) Reject(() => CliRequest.Parse(command), "fichier/session");
        var password = Guid.NewGuid().ToString("N"); var account = new AccountSettings(AppPaths.Sid, "test@example.invalid", "https://example.invalid", "uac", 15,
            AccountStore.Protect(AppPaths.Sid, password), "test", null, null);
        Check(AccountStore.Unprotect(account) == password, "DPAPI");
        var serialized = JsonSerializer.Serialize(account.Public(), AppPaths.Json); Check(!serialized.Contains(password) && !serialized.Contains(account.ProtectedPassword), "configuration publique");
        TestHello();
        Passkeys.PasskeyTests.Run();
        TransportTests.Run();
        ApplicationTests.Run();
        using (var transfer = RSA.Create(2048)) {
            var publicKey = Convert.ToBase64String(transfer.ExportSubjectPublicKeyInfo());
            var envelope = SealedSecret.Encrypt(password, publicKey);
            Check(!JsonSerializer.Serialize(envelope).Contains(password), "transfert sans texte en clair");
            var key = transfer.Decrypt(Convert.FromBase64String(envelope.EncryptedKey), RSAEncryptionPadding.OaepSHA256);
            var clear = new byte[Convert.FromBase64String(envelope.Data).Length];
            using var aes = new AesGcm(key, 16);
            aes.Decrypt(Convert.FromBase64String(envelope.Nonce), Convert.FromBase64String(envelope.Data), Convert.FromBase64String(envelope.Tag), clear);
            Check(Encoding.UTF8.GetString(clear) == password, "transfert vers moteur de saisie");
            CryptographicOperations.ZeroMemory(key); CryptographicOperations.ZeroMemory(clear);
        }
        using var stream = new MemoryStream(); PipeTransport.WriteAsync(stream, new Request("status", AppPaths.Element(new { })), CancellationToken.None).GetAwaiter().GetResult();
        stream.Position = 0; Check(PipeTransport.ReadAsync<Request>(stream, 1024, CancellationToken.None).GetAwaiter().GetResult().Op == "status", "trame IPC");
        using var bad = new MemoryStream(new byte[] { 255, 255, 255, 127 }); Reject(() => PipeTransport.ReadAsync<Request>(bad, 1024, CancellationToken.None).GetAwaiter().GetResult(), "taille IPC");
        Console.WriteLine("OK : delai, prolongation, commandes, DPAPI, secrets publics absents, signature Hello, IPC et passkeys CTAP/ES256/ABI Windows.");
    }
    private static void TestHello()
    {
        using var algorithm = ECDsa.Create(ECCurve.NamedCurves.nistP256); var key = algorithm.ExportParameters(false);
        var id = Convert.ToBase64String(RandomNumberGenerator.GetBytes(32));
        var credential = new HelloCredential(id, "EC2", Convert.ToBase64String(key.Q.X!), Convert.ToBase64String(key.Q.Y!));
        var challenge = HelloVerification.Base64Url(RandomNumberGenerator.GetBytes(32));
        var client = JsonSerializer.SerializeToUtf8Bytes(new { type = "webauthn.get", challenge, origin = AppPaths.Origin, crossOrigin = false });
        var auth = new byte[37]; SHA256.HashData(Encoding.UTF8.GetBytes(AppPaths.RpId)).CopyTo(auth, 0); auth[32] = 5;
        var signature = algorithm.SignData(auth.Concat(SHA256.HashData(client)).ToArray(), HashAlgorithmName.SHA256, DSASignatureFormat.Rfc3279DerSequence);
        var proof = new HelloProof(id, Convert.ToBase64String(client), Convert.ToBase64String(auth), Convert.ToBase64String(signature));
        HelloVerification.Verify(credential, proof, challenge);
        Reject(() => HelloVerification.Verify(credential, proof, "other"), "rejeu/autre challenge");
        auth[32] = 1; Reject(() => HelloVerification.Verify(credential, proof with { AuthenticatorData = Convert.ToBase64String(auth) }, challenge), "absence de vérification utilisateur");
        signature[0] ^= 1; Reject(() => HelloVerification.Verify(credential, proof with { Signature = Convert.ToBase64String(signature) }, challenge), "signature falsifiée");
        using var rsa = RSA.Create(2048); var rsaKey = rsa.ExportParameters(false);
        var rsaCredential = new HelloCredential(id, "RSA", Convert.ToBase64String(rsaKey.Modulus!), Convert.ToBase64String(rsaKey.Exponent!));
        var rsaSignature = rsa.SignData(Convert.FromBase64String(proof.AuthenticatorData).Concat(SHA256.HashData(client)).ToArray(), HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1);
        HelloVerification.Verify(rsaCredential, proof with { Signature = Convert.ToBase64String(rsaSignature) }, challenge);
    }
}
