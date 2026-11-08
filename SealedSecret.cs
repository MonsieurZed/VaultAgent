using System.Security.Cryptography;
using System.Text;

namespace Homelab.Vault;

internal sealed record SecretRequest(string ItemId, string Field);
internal sealed record SealedSecret(string EncryptedKey, string Nonce, string Tag, string Data)
{
    public static SealedSecret Encrypt(string value, string publicKey)
    {
        using var rsa = RSA.Create();
        var encoded = Convert.FromBase64String(publicKey);
        rsa.ImportSubjectPublicKeyInfo(encoded, out var consumed);
        if (consumed != encoded.Length || rsa.KeySize is < 2048 or > 4096) throw new ArgumentException("Clé de transfert invalide.");
        var key = RandomNumberGenerator.GetBytes(32); var clear = Encoding.UTF8.GetBytes(value);
        try {
            var nonce = RandomNumberGenerator.GetBytes(12); var tag = new byte[16]; var cipher = new byte[clear.Length];
            using var aes = new AesGcm(key, 16); aes.Encrypt(nonce, clear, cipher, tag);
            return new(Convert.ToBase64String(rsa.Encrypt(key, RSAEncryptionPadding.OaepSHA256)), Convert.ToBase64String(nonce), Convert.ToBase64String(tag), Convert.ToBase64String(cipher));
        }
        finally { CryptographicOperations.ZeroMemory(key); CryptographicOperations.ZeroMemory(clear); }
    }
}
