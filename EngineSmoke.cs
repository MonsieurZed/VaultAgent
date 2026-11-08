using System.Diagnostics;
using System.Text.Json;

namespace Homelab.Vault;

internal static class EngineSmoke
{
    public static void Run()
    {
        var temporary = Path.Combine(Path.GetTempPath(), "vault-engine-smoke-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(temporary);
        try {
            var settings = new AccountSettings(AppPaths.Sid, "test@example.invalid", "https://example.invalid", "uac", 15, "", "empty", null, null);
            using var engine = new EngineProcess();
            var clock = Stopwatch.StartNew(); engine.StartAsync(settings, new AccountStore(temporary), CancellationToken.None).GetAwaiter().GetResult();
            var startup = clock.Elapsed.TotalMilliseconds;
            var samples = new List<double>();
            for (var index = 0; index < 20; index++) {
                clock.Restart();
                var result = engine.RequestAsync(new { op = "call", method = "GET", path = "/status" }, CancellationToken.None).GetAwaiter().GetResult();
                if (result.GetProperty("status").GetString() != "unauthenticated") throw new InvalidOperationException("Le test doit utiliser un profil vide.");
                samples.Add(clock.Elapsed.TotalMilliseconds);
            }
            Console.WriteLine(JsonSerializer.Serialize(new { success = true, emptyProfile = true, samples = samples.Count,
                startupMs = Math.Round(startup), warmMeanMs = Math.Round(samples.Average(), 2), warmP95Ms = Math.Round(samples.OrderBy(v => v).ElementAt(18), 2) }, AppPaths.Json));
        }
        finally {
            if (!Path.GetFullPath(temporary).StartsWith(Path.GetTempPath(), StringComparison.OrdinalIgnoreCase)) throw new IOException("Répertoire de test invalide.");
            try { Directory.Delete(temporary, true); } catch (IOException) { }
        }
    }
}
