using System.Buffers.Binary;
using System.Formats.Cbor;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Homelab.Vault;

internal static class HelloVerification
{
    public static string Base64Url(byte[] data) => Convert.ToBase64String(data).TrimEnd('=').Replace('+', '-').Replace('/', '_');
    private static void CheckAuthenticator(byte[] data) {
        if (data.Length < 37 || !CryptographicOperations.FixedTimeEquals(data.AsSpan(0, 32), SHA256.HashData(Encoding.UTF8.GetBytes(AppPaths.RpId))) || (data[32] & 5) != 5)
            throw new InvalidOperationException("Preuve Windows Hello invalide.");
    }
    public static HelloCredential ReadRegistration(byte[] id, byte[] data)
    {
        CheckAuthenticator(data);
        if ((data[32] & 64) == 0 || data.Length < 55) throw new InvalidOperationException("Attestation incomplète.");
        var length = BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(53, 2));
        if (data.Length < 55 + length || !data.AsSpan(55, length).SequenceEqual(id)) throw new InvalidOperationException("Identifiant Hello incorrect.");
        var reader = new CborReader(data.AsMemory(55 + length), CborConformanceMode.Ctap2Canonical, allowMultipleRootLevelValues: true);
        var count = reader.ReadStartMap() ?? throw new InvalidOperationException("Clé Hello incorrecte.");
        var numbers = new Dictionary<int, int>(); var bytes = new Dictionary<int, byte[]>();
        for (var i = 0; i < count; i++) {
            var key = reader.ReadInt32();
            if (reader.PeekState() == CborReaderState.ByteString) bytes[key] = reader.ReadByteString();
            else numbers[key] = reader.ReadInt32();
        }
        reader.ReadEndMap();
        HelloCredential result;
        if (numbers.GetValueOrDefault(1) == 2 && numbers.GetValueOrDefault(3) == -7 && numbers.GetValueOrDefault(-1) == 1)
            result = new(Convert.ToBase64String(id), "EC2", Convert.ToBase64String(bytes[-2]), Convert.ToBase64String(bytes[-3]));
        else if (numbers.GetValueOrDefault(1) == 3 && numbers.GetValueOrDefault(3) == -257)
            result = new(Convert.ToBase64String(id), "RSA", Convert.ToBase64String(bytes[-1]), Convert.ToBase64String(bytes[-2]));
        else throw new InvalidOperationException("Algorithme Windows Hello non pris en charge.");
        ValidateKey(result);
        return result;
    }
    public static void ValidateKey(HelloCredential key)
    {
        if (Convert.FromBase64String(key.Id).Length is < 16 or > 4096) throw new InvalidOperationException("Identifiant Hello invalide.");
        var first = Convert.FromBase64String(key.First); var second = Convert.FromBase64String(key.Second);
        if (key.KeyType == "EC2" && first.Length == 32 && second.Length == 32) {
            using var algorithm = ECDsa.Create(new ECParameters { Curve = ECCurve.NamedCurves.nistP256, Q = new ECPoint { X = first, Y = second } });
        } else if (key.KeyType == "RSA" && first.Length is >= 256 and <= 512 && second.Length is >= 1 and <= 4) {
            using var algorithm = RSA.Create(); algorithm.ImportParameters(new RSAParameters { Modulus = first, Exponent = second });
        } else throw new InvalidOperationException("Clé Hello invalide.");
    }
    public static void Verify(HelloCredential key, HelloProof proof, string challenge)
    {
        ValidateKey(key);
        if (!CryptographicOperations.FixedTimeEquals(Convert.FromBase64String(proof.CredentialId), Convert.FromBase64String(key.Id))) throw new InvalidOperationException("Autre clé Hello.");
        var client = Convert.FromBase64String(proof.ClientData);
        using var document = JsonDocument.Parse(client);
        var json = document.RootElement;
        if (json.GetProperty("type").GetString() != "webauthn.get" || json.GetProperty("origin").GetString() != AppPaths.Origin ||
            json.GetProperty("challenge").GetString() != challenge || (json.TryGetProperty("crossOrigin", out var cross) && cross.GetBoolean())) throw new InvalidOperationException("Challenge Hello invalide.");
        var auth = Convert.FromBase64String(proof.AuthenticatorData);
        CheckAuthenticator(auth);
        var message = auth.Concat(SHA256.HashData(client)).ToArray();
        var signature = Convert.FromBase64String(proof.Signature);
        bool valid;
        if (key.KeyType == "EC2") {
            using var algorithm = ECDsa.Create(new ECParameters { Curve = ECCurve.NamedCurves.nistP256,
                Q = new ECPoint { X = Convert.FromBase64String(key.First), Y = Convert.FromBase64String(key.Second) } });
            valid = algorithm.VerifyData(message, signature, HashAlgorithmName.SHA256, DSASignatureFormat.Rfc3279DerSequence);
        } else {
            using var algorithm = RSA.Create(); algorithm.ImportParameters(new RSAParameters { Modulus = Convert.FromBase64String(key.First), Exponent = Convert.FromBase64String(key.Second) });
            valid = algorithm.VerifyData(message, signature, HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1);
        }
        if (!valid) throw new InvalidOperationException("Signature Windows Hello refusée.");
    }
}
