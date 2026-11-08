namespace Homelab.Vault.Passkeys;

internal sealed record ProviderPublicKeys(byte[] OperationKey, byte[]? VerificationKey);
internal sealed record SignedPasskeyRequest(Guid TransactionId, bool Create, byte[] Data, byte[] Signature);
internal sealed record PasskeyCommit(string JobId, byte[] VerificationSignature, string? CredentialId);
internal sealed record StoredPasskey(string ItemId, PasskeyCredential Credential);
internal sealed record PasskeyChoice(string CredentialId, string UserName, string DisplayName);
internal sealed record PendingPasskey(SignedPasskeyRequest Message, PasskeyRequest Request, byte[] Challenge,
    DateTimeOffset Expires, uint ProcessId, uint SessionId, int Generation, bool Cancelled = false);
