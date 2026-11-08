using System.Collections.Concurrent;
using System.IO.Pipes;
using System.ServiceProcess;
using System.Text.Json;

namespace Homelab.Vault;

internal sealed class BrokerService : ServiceBase
{
    private readonly CancellationTokenSource stop = new();
    private readonly ConcurrentDictionary<string, VaultSession> accounts = new();
    private readonly AccountStore store = new(AppPaths.Data);
    private Task? listener;
    public BrokerService() { ServiceName = AppPaths.ServiceName; CanShutdown = true; CanHandleSessionChangeEvent = true; AutoLog = false; }
    protected override void OnStart(string[] args) { Directory.CreateDirectory(AppPaths.Data); listener = RunAsync(stop.Token); }
    protected override void OnStop() { stop.Cancel(); foreach (var account in accounts.Values) account.Lock(); try { listener?.Wait(5000); } catch { } }
    protected override void OnShutdown() => OnStop();
    protected override void OnSessionChange(SessionChangeDescription change)
    {
        if (change.Reason is SessionChangeReason.SessionLock or SessionChangeReason.SessionLogoff or SessionChangeReason.RemoteDisconnect or SessionChangeReason.ConsoleDisconnect)
            foreach (var account in accounts.Values.Where(a => a.SessionId == change.SessionId)) account.Lock();
    }
    private async Task RunAsync(CancellationToken token)
    {
        using var timer = new System.Threading.Timer(_ => { foreach (var account in accounts.Values) account.ExpireIfNeeded(); }, null, 500, 500);
        while (!token.IsCancellationRequested) {
            NamedPipeServerStream? pipe = null;
            try {
                pipe = PipeTransport.CreateServer();
                await pipe.WaitForConnectionAsync(token);
                _ = HandleAsync(pipe, token);
                pipe = null;
            }
            catch (OperationCanceledException) { break; }
            catch { await Task.Delay(500, token); }
            finally { pipe?.Dispose(); }
        }
    }
    private async Task HandleAsync(NamedPipeServerStream pipe, CancellationToken shutdown)
    {
        using (pipe) using (var timeout = CancellationTokenSource.CreateLinkedTokenSource(shutdown)) {
            timeout.CancelAfter(TimeSpan.FromMinutes(2));
            var stage = "read";
            try {
                Request request;
                using (var reading = CancellationTokenSource.CreateLinkedTokenSource(timeout.Token)) {
                    reading.CancelAfter(TimeSpan.FromSeconds(5));
                    request = await PipeTransport.ReadAsync<Request>(pipe, 4 * 1024 * 1024, reading.Token);
                }
                stage = "identity";
                var caller = PipeTransport.Identify(pipe);
                stage = "account";
                var sid = request.OwnerSid ?? caller.Sid;
                if (sid != caller.Sid) throw new UnauthorizedAccessException("Utiliser le même compte Windows.");
                var account = accounts.GetOrAdd(sid, id => new VaultSession(id, store));
                stage = "dispatch";
                object response;
                switch (request.Op) {
                    case "passkey-metadata":
                        if (!ProcessIdentity.IsPasskeyProvider((int)caller.ProcessId)) throw new UnauthorizedAccessException("Fournisseur Windows protégé requis.");
                        response = await account.PasskeyMetadataAsync(caller, timeout.Token); break;
                    case "passkey-inspect":
                        if (!ProcessIdentity.IsPasskeyProvider((int)caller.ProcessId)) throw new UnauthorizedAccessException("Fournisseur Windows protégé requis.");
                        response = account.InspectPasskey(request.Payload.Deserialize<Passkeys.SignedPasskeyRequest>(AppPaths.Json) ?? throw new ArgumentException("Requête de passkey invalide.")); break;
                    case "passkey-register":
                        if (!ProcessIdentity.IsPasskeyProvider((int)caller.ProcessId)) throw new UnauthorizedAccessException("Composant protégé de passkeys requis.");
                        account.RegisterPasskeyProvider(request.Payload.Deserialize<Passkeys.ProviderPublicKeys>(AppPaths.Json) ?? throw new ArgumentException("Clés publiques invalides.")); response = new { registered = true }; break;
                    case "passkey-begin":
                        if (!ProcessIdentity.IsPasskeyProvider((int)caller.ProcessId)) throw new UnauthorizedAccessException("Composant protégé de passkeys requis.");
                        response = await account.BeginPasskeyAsync(request.Payload.Deserialize<Passkeys.SignedPasskeyRequest>(AppPaths.Json) ?? throw new ArgumentException("Requête de passkey invalide."), caller, timeout.Token); break;
                    case "passkey-complete":
                        if (!ProcessIdentity.IsPasskeyProvider((int)caller.ProcessId)) throw new UnauthorizedAccessException("Composant protégé de passkeys requis.");
                        response = await account.CompletePasskeyAsync(request.Payload.Deserialize<Passkeys.PasskeyCommit>(AppPaths.Json) ?? throw new ArgumentException("Validation de passkey invalide."), caller, timeout.Token); break;
                    case "passkey-cancel":
                        account.CancelPasskey(request.Payload.GetProperty("jobId").GetString()!, caller); response = new { cancelled = true }; break;
                    case "status": response = account.Status(); break;
                    case "settings": response = account.PublicSettings(); break;
                    case "lock": account.Lock(); response = new { unlocked = false }; break;
                    case "forget":
                        if (!caller.Elevated) throw new UnauthorizedAccessException("Validation UAC requise.");
                        account.Forget(); response = new { configured = false }; break;
                    case "challenge": response = account.BeginHello(); break;
                    case "unlock-hello":
                        await account.UnlockAsync(false, request.Payload.Deserialize<HelloProof>(AppPaths.Json), caller.SessionId, timeout.Token);
                        response = account.Status(); break;
                    case "unlock-uac":
                        if (!caller.Elevated) throw new UnauthorizedAccessException("Validation UAC requise.");
                        await account.UnlockAsync(true, null, caller.SessionId, timeout.Token); response = account.Status(); break;
                    case "configure":
                        if (!caller.Elevated) throw new UnauthorizedAccessException("Configuration administrateur requise.");
                        await account.ConfigureAsync(request.Payload.Deserialize<ConfigureAccount>(AppPaths.Json) ?? throw new ArgumentException("Compte invalide."), caller.SessionId, timeout.Token);
                        response = account.Status(); break;
                    case "execute":
                        account.RequireAccess(caller);
                        response = await account.ExecuteAsync(request.Payload.Deserialize<string[]>(AppPaths.Json) ?? Array.Empty<string>(), timeout.Token); break;
                    case "catalog":
                        response = await account.CatalogAsync(request.Payload.TryGetProperty("search", out var search) ? search.GetString() ?? "" : "", caller, timeout.Token); break;
                    case "seal":
                        var secretRequest = request.Payload.GetProperty("request").Deserialize<SecretRequest>(AppPaths.Json) ?? throw new ArgumentException("Champ invalide.");
                        response = SealedSecret.Encrypt(await account.SecretAsync(secretRequest, caller, timeout.Token), request.Payload.GetProperty("publicKey").GetString()!); break;
                    case "worker-secret":
                        if (!ProcessIdentity.IsInstalled((int)caller.ProcessId, "VaultAgent.exe")) throw new UnauthorizedAccessException("Composant de saisie protégé requis.");
                        response = new { value = await account.SecretAsync(request.Payload.Deserialize<SecretRequest>(AppPaths.Json) ?? throw new ArgumentException("Champ invalide."), caller, timeout.Token) }; break;
                    default: throw new ArgumentException("Opération inconnue.");
                }
                await PipeTransport.WriteAsync(pipe, Reply.Ok(response), timeout.Token);
            }
            catch (Exception error) {
                try { File.WriteAllText(Path.Combine(AppPaths.Install, "service-diagnostic.json"), JsonSerializer.Serialize(new {
                    stage, type = error.GetType().Name, code = error.HResult.ToString("X8"),
                    nativeCode = error is System.ComponentModel.Win32Exception native ? native.NativeErrorCode : 0,
                    identityError = stage == "identity" ? error.Message : null, date = DateTimeOffset.UtcNow
                    , identityFile = stage == "identity" && error is FileNotFoundException missing ? missing.FileName : null
                }, AppPaths.Json)); } catch { }
                var message = error is OperationCanceledException ? "Délai de réponse dépassé." :
                    error is (ArgumentException or InvalidOperationException or UnauthorizedAccessException) ? error.Message : "Service du coffre indisponible.";
                try { await PipeTransport.WriteAsync(pipe, Reply.Fail(message), CancellationToken.None); } catch { }
            }
        }
    }
}
