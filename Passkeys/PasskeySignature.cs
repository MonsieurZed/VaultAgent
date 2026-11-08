using System.Security.Cryptography;

namespace Homelab.Vault.Passkeys;

internal static class PasskeySignature
{
    public static void ValidateKey(byte[] encoded) {
        using var key = Import(encoded);
        if (key.AlgorithmGroup is null || key.AlgorithmGroup != CngAlgorithmGroup.ECDsa && key.AlgorithmGroup != CngAlgorithmGroup.Rsa) throw new CryptographicException("Algorithme de signature Windows non pris en charge.");
        if (key.AlgorithmGroup == CngAlgorithmGroup.Rsa && key.KeySize < 2048 || key.AlgorithmGroup == CngAlgorithmGroup.ECDsa && key.KeySize != 256) throw new CryptographicException("Taille de clé Windows invalide.");
    }
    private static CngKey Import(byte[] encoded) {
        if (encoded.Length is < 16 or > 8192) throw new CryptographicException("Clé publique Windows invalide.");
        return CngKey.Import(encoded, CngKeyBlobFormat.GenericPublicBlob);
    }
    public static void Verify(byte[] encoded, byte[] hash, byte[] signature) {
        if (hash.Length != 32 || signature.Length is < 64 or > 1024) throw new CryptographicException("Signature Windows invalide.");
        ValidateKey(encoded); using var key = Import(encoded); bool valid;
        if (key.AlgorithmGroup == CngAlgorithmGroup.ECDsa) { using var verifier = new ECDsaCng(key); valid = verifier.VerifyHash(hash, signature, DSASignatureFormat.IeeeP1363FixedFieldConcatenation); }
        else { using var verifier = new RSACng(key); valid = verifier.VerifyHash(hash, signature, HashAlgorithmName.SHA256, RSASignaturePadding.Pss) || verifier.VerifyHash(hash, signature, HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1); }
        if (!valid) throw new UnauthorizedAccessException("Signature de la plateforme ou validation Hello refusée.");
    }
}
