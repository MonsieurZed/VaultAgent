using System.Diagnostics;
using System.Text.Json;

namespace Homelab.Vault;

internal sealed class EngineProcess : IDisposable
{
    private Process? process;
    private long sequence;
    private readonly SemaphoreSlim serial = new(1, 1);
    public bool IsAlive { get { try { return process is { HasExited: false }; } catch { return false; } } }
    public async Task StartAsync(AccountSettings settings, AccountStore store, CancellationToken token)
    {
        await StartProcessAsync(settings, store, token);
        var prepared = await RequestAsync(new { op = "prepare", email = settings.Email, server = settings.Server }, token);
        if (prepared.GetProperty("restartRequired").GetBoolean()) {
            Kill();
            await StartProcessAsync(settings, store, token);
            var confirmation = await RequestAsync(new { op = "prepare", email = settings.Email, server = settings.Server }, token);
            if (confirmation.GetProperty("restartRequired").GetBoolean()) throw new IOException("Configuration du serveur non persistée.");
        }
    }
    private async Task StartProcessAsync(AccountSettings settings, AccountStore store, CancellationToken token)
    {
        var info = new ProcessStartInfo(Path.Combine(AppPaths.Install, "runtime", "node.exe")) {
            UseShellExecute = false, CreateNoWindow = true,
            RedirectStandardInput = true, RedirectStandardOutput = true, RedirectStandardError = true,
            WorkingDirectory = Path.Combine(AppPaths.Install, "runtime", "engine") };
        info.ArgumentList.Add(Path.Combine(info.WorkingDirectory, "bw.js"));
        foreach (var name in info.Environment.Keys.Where(k => k.StartsWith("NODE_", StringComparison.OrdinalIgnoreCase) || k.StartsWith("BW_", StringComparison.OrdinalIgnoreCase)).ToArray()) info.Environment.Remove(name);
        info.Environment["BITWARDENCLI_APPDATA_DIR"] = store.ProfileFor(settings);
        info.Environment["BW_NOINTERACTION"] = "true";
        if (settings.CertificatePem != null) {
            var certificate = Path.Combine(store.DirectoryFor(settings.Sid), "certificate-" + settings.ProfileId + ".pem");
            File.WriteAllText(certificate, settings.CertificatePem);
            info.Environment["NODE_EXTRA_CA_CERTS"] = certificate;
        }
        process = Process.Start(info) ?? throw new InvalidOperationException("Démarrage du moteur impossible.");
        _ = DrainErrorsAsync(process);
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(token);
        timeout.CancelAfter(TimeSpan.FromSeconds(60));
        var line = await process.StandardOutput.ReadLineAsync(timeout.Token);
        using var response = JsonDocument.Parse(line ?? throw new IOException("Moteur arrêté."));
        if (!response.RootElement.TryGetProperty("ready", out var ready) || !ready.GetBoolean()) throw new IOException("Initialisation du moteur impossible.");
    }
    private static async Task DrainErrorsAsync(Process child)
    {
        var buffer = new char[2048];
        try { while (await child.StandardError.ReadAsync(buffer) != 0) { } } catch { }
    }
    public async Task<JsonElement> RequestAsync(object request, CancellationToken token)
    {
        await serial.WaitAsync(token);
        try {
            var child = process;
            if (child == null || child.HasExited) throw new IOException("Moteur non disponible.");
            var payload = JsonSerializer.SerializeToNode(request, AppPaths.Json)!.AsObject();
            var id = Interlocked.Increment(ref sequence);
            payload["id"] = id;
            await child.StandardInput.WriteLineAsync(payload.ToJsonString().AsMemory(), token);
            await child.StandardInput.FlushAsync(token);
            var line = await child.StandardOutput.ReadLineAsync(token);
            using var document = JsonDocument.Parse(line ?? throw new IOException("Moteur arrêté."));
            var response = document.RootElement;
            if (!response.TryGetProperty("id", out var actual) || actual.GetInt64() != id) throw new IOException("Réponse du moteur invalide.");
            if (!response.GetProperty("success").GetBoolean()) {
                var code = response.TryGetProperty("code", out var reason) ? reason.GetString() : null;
                var message = code switch {
                    "login:two-factor" => "Le serveur demande un code 2FA. Saisir un code authentificateur récent.",
                    "login:credentials" => "Connexion refusée : vérifier l'adresse email et le mot de passe maître de ce coffre.",
                    "login:captcha" => "Le serveur demande une vérification CAPTCHA ; connexion interactive nécessaire.",
                    "login:tls" => "Le moteur ne valide pas le certificat HTTPS du coffre.",
                    "login:network" => "Le moteur ne parvient pas à joindre le serveur HTTPS du coffre.",
                    "login:engine-api" or "login:engine-type" => "Erreur interne du moteur de connexion Bitwarden.",
                    _ => "Bitwarden a refusé l'opération. Vérifier le compte et le code 2FA éventuel."
                };
                throw new InvalidOperationException(message + (code is { Length: < 60 } && System.Text.RegularExpressions.Regex.IsMatch(code, "^[a-z-]+:[a-z-]+$") ? " [" + code + "]" : ""));
            }
            return response.TryGetProperty("data", out var data) ? data.Clone() : AppPaths.Element(new { });
        }
        finally { serial.Release(); }
    }
    public void Kill()
    {
        var child = Interlocked.Exchange(ref process, null);
        if (child == null) return;
        try { if (!child.HasExited) child.Kill(true); } catch { }
        child.Dispose();
    }
    public void Dispose() { Kill(); }
}
