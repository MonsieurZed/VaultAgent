using System.Security.Cryptography;
using System.Text.Json;
using Homelab.Vault.Passkeys;

namespace Homelab.Vault;

internal sealed partial class VaultSession
{
    private readonly Dictionary<string, PendingPasskey> passkeyJobs = new();
    private readonly Dictionary<string, DateTimeOffset> consumedPasskeys = new();
    private string ProviderKeyFile => Path.Combine(store.DirectoryFor(sid), "provider-public-keys.json");
    private ProviderPublicKeys ProviderKeys() => JsonSerializer.Deserialize<ProviderPublicKeys>(File.ReadAllText(ProviderKeyFile), AppPaths.Json) ?? throw new InvalidOperationException("Enregistrer le fournisseur de passkeys.");
    public object InspectPasskey(SignedPasskeyRequest message) {
        if (message.Data.Length is 0 or > 65536 || message.TransactionId == Guid.Empty) throw new ArgumentException("Requête de passkey invalide.");
        PasskeySignature.Verify(ProviderKeys().OperationKey, SHA256.HashData(message.Data), message.Signature);
        var request = CtapRequest.Read(message.Data, message.Create);
        return new { rpId = request.RpId, create = message.Create };
    }
    public void RegisterPasskeyProvider(ProviderPublicKeys keys)
    {
        if (settings == null) throw new InvalidOperationException("Configurer le coffre avant le fournisseur de passkeys.");
        PasskeySignature.ValidateKey(keys.OperationKey); if (keys.VerificationKey != null) PasskeySignature.ValidateKey(keys.VerificationKey);
        lock (stateGate) {
            Directory.CreateDirectory(store.DirectoryFor(sid));
            var temporary = ProviderKeyFile + ".tmp";
            File.WriteAllText(temporary, JsonSerializer.Serialize(keys, AppPaths.Json)); File.Move(temporary, ProviderKeyFile, true); passkeyJobs.Clear();
        }
    }
    private async Task<StoredPasskey[]> PasskeysForAsync(string? rpId, CancellationToken token) {
        var result = await engine!.RequestAsync(new { op = "passkey-list", rpId }, token);
        return result.Deserialize<StoredPasskey[]>(AppPaths.Json) ?? Array.Empty<StoredPasskey>();
    }
    public async Task<object> PasskeyMetadataAsync(Caller caller, CancellationToken token) {
        await serial.WaitAsync(token);
        try { RequireAccess(caller); var keys = await PasskeysForAsync(null, token); return keys.Select(value => new {
            credentialId = value.Credential.IdBytes(), rpId = value.Credential.RpId, rpName = value.Credential.RpName,
            userId = PasskeyCredential.Decode(value.Credential.UserHandle), userName = value.Credential.UserName, displayName = value.Credential.UserDisplayName }).ToArray(); }
        finally { serial.Release(); }
    }
    public async Task<object> BeginPasskeyAsync(SignedPasskeyRequest message, Caller caller, CancellationToken token)
    {
        await serial.WaitAsync(token);
        try {
            RequireAccess(caller);
            var keys = ProviderKeys();
            if (keys.VerificationKey == null) throw new InvalidOperationException("Configurer Windows Hello puis actualiser le fournisseur de passkeys.");
            var digest = SHA256.HashData(message.Data); PasskeySignature.Verify(keys.OperationKey, digest, message.Signature);
            var request = CtapRequest.Read(message.Data, message.Create);
            if (message.TransactionId == Guid.Empty || request.RpId == WindowsPasskeyApi.RpId || request.RpId == AppPaths.RpId) throw new UnauthorizedAccessException("Requête de passkey réservée ou invalide.");
            var credentials = await PasskeysForAsync(request.RpId, token);
            var choices = message.Create ? Array.Empty<PasskeyChoice>() : credentials.Where(value =>
                (request.Credentials.Count == 0 ? value.Credential.Discoverable == "true" : request.Credentials.Any(id => id.AsSpan().SequenceEqual(value.Credential.IdBytes()))))
                .Select(value => new PasskeyChoice(value.Credential.CredentialId, value.Credential.UserName, value.Credential.UserDisplayName)).ToArray();
            if (!message.Create && choices.Length == 0) throw new InvalidOperationException("Aucune passkey correspondante dans le coffre.");
            var jobId = PasskeyCredential.Encode(RandomNumberGenerator.GetBytes(32));
            var challenge = SHA256.HashData(digest.Concat(RandomNumberGenerator.GetBytes(32)).ToArray());
            lock (stateGate) {
                var now = DateTimeOffset.UtcNow;
                foreach (var id in passkeyJobs.Where(pair => pair.Value.Expires < now).Select(pair => pair.Key).ToArray()) passkeyJobs.Remove(id);
                foreach (var id in consumedPasskeys.Where(pair => pair.Value < now).Select(pair => pair.Key).ToArray()) consumedPasskeys.Remove(id);
                if (passkeyJobs.Count >= 8 || consumedPasskeys.Count >= 1024 || consumedPasskeys.ContainsKey(Convert.ToHexString(digest))) throw new UnauthorizedAccessException("Requête de passkey répétée ou trop de requêtes en attente.");
                passkeyJobs.Add(jobId, new(message, request, challenge, now.AddMinutes(2), caller.ProcessId, caller.SessionId, generation));
            }
            lease.Touch();
            return new { jobId, challenge, create = message.Create, rpId = request.RpId, rpName = request.RpName, userName = request.UserName, displayName = request.DisplayName, choices };
        }
        finally { serial.Release(); }
    }
    public void CancelPasskey(string jobId, Caller caller) {
        lock (stateGate) { if (passkeyJobs.TryGetValue(jobId, out var job) && job.ProcessId == caller.ProcessId && job.SessionId == caller.SessionId) passkeyJobs.Remove(jobId); }
    }
    public async Task<object> CompletePasskeyAsync(PasskeyCommit commit, Caller caller, CancellationToken token)
    {
        await serial.WaitAsync(token);
        try {
            RequireAccess(caller); PendingPasskey job;
            lock (stateGate) {
                if (!passkeyJobs.Remove(commit.JobId, out job!) || job.Expires < DateTimeOffset.UtcNow || job.Generation != generation || job.ProcessId != caller.ProcessId || job.SessionId != caller.SessionId) throw new UnauthorizedAccessException("Opération de passkey expirée, annulée ou différente.");
            }
            PasskeySignature.Verify(ProviderKeys().VerificationKey ?? throw new InvalidOperationException("Windows Hello requis."), job.Challenge, commit.VerificationSignature);
            lock (stateGate) {
                var digest = Convert.ToHexString(SHA256.HashData(job.Message.Data));
                if (consumedPasskeys.ContainsKey(digest)) throw new UnauthorizedAccessException("Requête de passkey déjà utilisée.");
                consumedPasskeys.Add(digest, DateTimeOffset.UtcNow.AddMinutes(10));
            }
            var credentials = await PasskeysForAsync(job.Request.RpId, token);
            PasskeyCredential credential;
            if (job.Message.Create) {
                if (credentials.Any(value => job.Request.Credentials.Any(id => id.AsSpan().SequenceEqual(value.Credential.IdBytes())))) throw new InvalidOperationException("Une passkey exclue existe déjà pour ce compte.");
                credential = PasskeyCredential.Create(job.Request);
                var saved = await engine!.RequestAsync(new { op = "passkey-create", credential }, token);
                if (!saved.GetProperty("verified").GetBoolean()) throw new InvalidOperationException("Enregistrement de passkey non vérifié.");
            } else {
                credential = credentials.Select(value => value.Credential).FirstOrDefault(value => value.CredentialId == commit.CredentialId &&
                    (job.Request.Credentials.Count == 0 ? value.Discoverable == "true" : job.Request.Credentials.Any(id => id.AsSpan().SequenceEqual(value.IdBytes())))) ?? throw new UnauthorizedAccessException("Passkey sélectionnée différente de la requête.");
            }
            RequireAccess(caller);
            lock (stateGate) { if (job.Generation != generation) throw new UnauthorizedAccessException("Opération annulée par un verrouillage."); }
            var response = job.Message.Create ? credential.Attestation() : credential.Assertion(job.Request); lease.Touch();
            return new { response, credentialId = credential.IdBytes(), rpId = credential.RpId, rpName = credential.RpName,
                userId = PasskeyCredential.Decode(credential.UserHandle), userName = credential.UserName, displayName = credential.UserDisplayName };
        }
        finally { serial.Release(); }
    }
}
