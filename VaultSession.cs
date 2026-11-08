using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text.Json;

namespace Homelab.Vault;

internal sealed partial class VaultSession(string sid, AccountStore store) : IDisposable
{
    private readonly SemaphoreSlim serial = new(1, 1);
    private EngineProcess? engine;
    private readonly SlidingLease lease = new();
    private AccountSettings? settings = store.Load(sid);
    private string? challenge;
    private DateTimeOffset challengeExpiry;
    private readonly object stateGate = new();
    private int generation;
    public uint SessionId { get; private set; }
    public bool IsUnlocked => lease.IsValid && engine?.IsAlive == true;
    public object Status() => new { configured = settings != null, unlocked = IsUnlocked, expiresUtc = IsUnlocked ? lease.ExpiresUtc : null,
        timeoutMinutes = settings?.TimeoutMinutes, mode = settings?.Mode ?? "uac", email = settings?.Email, server = settings?.Server,
        enginePersistent = true, version = typeof(VaultSession).Assembly.GetName().Version!.ToString(3) };
    public object PublicSettings() => settings?.Public() ?? new { configured = false };
    public void ExpireIfNeeded() { if (lease.Authorized && (!lease.IsValid || engine?.IsAlive != true)) Lock(); }
    private int Reset()
    {
        EngineProcess? previous; int version;
        lock (stateGate) { version = ++generation; lease.Close(); previous = engine; engine = null; challenge = null; passkeyJobs.Clear(); }
        previous?.Dispose(); return version;
    }
    public void Lock() => Reset();
    public void Forget() { Lock(); lock (stateGate) { store.Forget(sid); settings = null; } }
    private async Task<EngineProcess> OpenEngineAsync(AccountSettings account, string? code, CancellationToken token)
    {
        var candidate = new EngineProcess();
        try {
            await candidate.StartAsync(account, store, token);
            await candidate.RequestAsync(new { op = "connect", email = account.Email, server = account.Server,
                password = AccountStore.Unprotect(account), code }, token);
            return candidate;
        }
        catch { candidate.Dispose(); throw; }
    }
    public async Task ConfigureAsync(ConfigureAccount value, uint sessionId, CancellationToken token)
    {
        await serial.WaitAsync(token);
        try {
            if (!Uri.TryCreate(value.Server.TrimEnd('/'), UriKind.Absolute, out var server) || server.Scheme != "https" || !string.IsNullOrEmpty(server.UserInfo)) throw new ArgumentException("Adresse HTTPS du serveur requise.");
            if (string.IsNullOrWhiteSpace(value.Email) || value.Email.Length > 320 || !value.Email.Contains('@')) throw new ArgumentException("Adresse email invalide.");
            if (value.Mode is not ("uac" or "hello" or "hello-or-uac")) throw new ArgumentException("Mode de déverrouillage invalide.");
            if (value.TimeoutMinutes is < 1 or > 1440) throw new ArgumentException("Délai entre 1 et 1440 minutes, ou désactivé.");
            var credential = value.Hello ?? settings?.Hello;
            if (value.Mode != "uac") { if (credential == null) throw new ArgumentException("Enregistrer Windows Hello avant de valider."); HelloVerification.ValidateKey(credential); }
            var certificate = value.CertificatePem ?? settings?.CertificatePem;
            if (!string.IsNullOrWhiteSpace(certificate)) { using var parsed = X509Certificate2.CreateFromPem(certificate); } else certificate = null;
            if (string.IsNullOrEmpty(value.Password) && settings == null) throw new ArgumentException("Mot de passe maître requis.");
            var accountChanged = settings == null || !string.Equals(settings.Email, value.Email.Trim(), StringComparison.OrdinalIgnoreCase) || settings.Server != server.AbsoluteUri.TrimEnd('/');
            if (accountChanged && string.IsNullOrEmpty(value.Password)) throw new ArgumentException("Saisir le mot de passe du nouveau compte.");
            var account = new AccountSettings(sid, value.Email.Trim(), server.AbsoluteUri.TrimEnd('/'), value.Mode, value.TimeoutMinutes,
                string.IsNullOrEmpty(value.Password) ? settings!.ProtectedPassword : AccountStore.Protect(sid, value.Password),
                accountChanged || !string.IsNullOrEmpty(value.Password) ? Guid.NewGuid().ToString("N") : settings!.ProfileId, credential, certificate,
                value.BlindMode);
            var version = Reset();
            var candidate = await OpenEngineAsync(account, value.TwoFactorCode, token);
            try {
                lock (stateGate) {
                    if (version != generation) throw new InvalidOperationException("Ouverture annulée par un verrouillage.");
                    store.Save(account); settings = account; engine = candidate; SessionId = sessionId; lease.Open(account.TimeoutMinutes);
                }
            }
            catch { candidate.Dispose(); throw; }
        }
        finally { serial.Release(); }
    }
    public object BeginHello()
    {
        var account = settings ?? throw new InvalidOperationException("Configurer le compte d'abord.");
        if (account.Mode == "uac" || account.Hello == null) throw new InvalidOperationException("Windows Hello désactivé pour ce compte.");
        lock (stateGate) {
            challenge = HelloVerification.Base64Url(RandomNumberGenerator.GetBytes(32)); challengeExpiry = DateTimeOffset.UtcNow.AddMinutes(1);
            return new { challenge, credential = account.Hello };
        }
    }
    public async Task UnlockAsync(bool elevated, HelloProof? proof, uint sessionId, CancellationToken token)
    {
        await serial.WaitAsync(token);
        try {
            var account = settings ?? throw new InvalidOperationException("Configurer le compte d'abord.");
            if (elevated) {
                if (account.Mode == "hello") throw new UnauthorizedAccessException("Ce compte demande Windows Hello.");
            } else {
                string? pending; DateTimeOffset expires;
                lock (stateGate) { pending = challenge; expires = challengeExpiry; challenge = null; }
                if (proof == null || account.Mode == "uac" || account.Hello == null || pending == null || DateTimeOffset.UtcNow > expires) throw new UnauthorizedAccessException("Autorisation Windows requise.");
                HelloVerification.Verify(account.Hello, proof, pending);
            }
            var version = Reset();
            var candidate = await OpenEngineAsync(account, null, token);
            lock (stateGate) {
                if (version != generation) { candidate.Dispose(); throw new InvalidOperationException("Ouverture annulée par un verrouillage."); }
                engine = candidate; SessionId = sessionId; lease.Open(account.TimeoutMinutes);
            }
        }
        finally { serial.Release(); }
    }
    public async Task<JsonElement> ExecuteAsync(string[] args, CancellationToken token)
    {
        var call = CliRequest.Parse(args);
        await serial.WaitAsync(token);
        try {
            if (!IsUnlocked || engine == null) { Lock(); throw new UnauthorizedAccessException("Coffre verrouillé."); }
            if (settings!.BlindMode && !(args[0] is "status" or "sync" || args[0] == "list" && args[1] == "folders" || args[0] == "get" && args[1] == "template"))
                throw new UnauthorizedAccessException("Mode sans lecture : utiliser catalog ou fill. L'export des secrets est désactivé.");
            lease.Touch();
            return await engine.RequestAsync(call.Message(), token);
        }
        finally { serial.Release(); }
    }
    public void Dispose() { Lock(); }
}
