using System.ServiceProcess;
using System.Text.Json;

namespace Homelab.Vault;

internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        try {
            if (args.Contains("--service")) { ServiceBase.Run(new BrokerService()); return 0; }
            if (args.Contains("--self-test")) { SelfTests.Run(); return 0; }
            if (args.Contains("--engine-smoke")) { EngineSmoke.Run(); return 0; }
            if (args.Contains("--capabilities")) { Print(AppPaths.Element(new { helloAvailable = HelloNative.Available, webauthnApiVersion = HelloNative.ApiVersion, directInput = true, browserAdapter = true, vaultPasskeyProvider = false })); return 0; }
            ApplicationConfiguration.Initialize();
            if (args.Contains("--write-icons")) { ShieldIcon.WriteAssets(args[^1]); return 0; }
            if (args.Length == 2 && args[0] == "--render-ui") { UiTests.Render(args[1]); return 0; }
            if (args.Length == 2 && args[0] == "--input-test") { UiTests.Input(args[1]); return 0; }
            var ownerIndex = Array.IndexOf(args, "--owner");
            if (ownerIndex >= 0 && (ownerIndex + 1 >= args.Length || args[ownerIndex + 1] != AppPaths.Sid)) throw new UnauthorizedAccessException("Utiliser le même compte Windows pour l'UAC.");
            if (args.Contains("--settings")) { Application.Run(new AccountForm()); return 0; }
            if (args.Contains("--unlock-uac")) { PipeTransport.SendAsync("unlock-uac").GetAwaiter().GetResult(); return 0; }
            if (args.Contains("--forget")) { PipeTransport.SendAsync("forget").GetAwaiter().GetResult(); return 0; }
            if (args.Length > 0 && !args.Contains("--tray")) return RunCli(args);
            using var mutex = new Mutex(false, "Local\\HomelabVaultAgentTray-" + AppPaths.Sid);
            var owned = false; try { owned = mutex.WaitOne(0); } catch (AbandonedMutexException) { owned = true; }
            if (!owned) return 0;
            try { Application.Run(new TrayController()); } finally { mutex.ReleaseMutex(); }
            return 0;
        }
        catch (Exception error) {
            if (args.Contains("--service")) return 1;
            if (args.Contains("--self-test") || args.Contains("--engine-smoke")) { Console.Error.WriteLine(error.Message); return 1; }
            if (args.Length > 0 && args[0] is "status" or "lock" or "unlock" or "exec" or "catalog" or "type" or "seal" or "seal-probe") { Console.Error.WriteLine(error.Message); return 1; }
            ApplicationConfiguration.Initialize();
            MessageBox.Show(error.Message, "Coffre Bitwarden", MessageBoxButtons.OK, MessageBoxIcon.Error); return 1;
        }
    }
    private static int RunCli(string[] args)
    {
        if (args[0] == "status") { Print(PipeTransport.SendAsync("status").GetAwaiter().GetResult()); return 0; }
        if (args[0] == "lock") { PipeTransport.SendAsync("lock").GetAwaiter().GetResult(); Console.WriteLine("Verrouillé."); return 0; }
        if (args[0] == "unlock") return UnlockFlow.RunDialog() ? 0 : 1;
        if (args[0] == "seal-probe" && args.Length == 2) { Print(AppPaths.Element(SealedSecret.Encrypt("vault-agent-paste-probe-20261107", args[1]))); return 0; }
        if (args[0] is "catalog" or "type" or "seal") {
            var targetWindow = DirectInput.GetForegroundWindow();
            var status = PipeTransport.SendAsync("status").GetAwaiter().GetResult();
            if (!status.GetProperty("unlocked").GetBoolean() && !UnlockFlow.RunDialog()) return 1;
            if (args[0] == "catalog") { Print(PipeTransport.SendAsync("catalog", new { search = args.Length > 1 ? args[1] : "" }).GetAwaiter().GetResult()); return 0; }
            if (args[0] == "seal" && args.Length == 4) {
                Print(PipeTransport.SendAsync("seal", new { request = new SecretRequest(args[1], args[2]), publicKey = args[3] }).GetAwaiter().GetResult()); return 0;
            }
            if (args[0] != "type" || args.Length != 3) throw new ArgumentException("Usage : type <id-fiche> <username|password|totp>.");
            var result = PipeTransport.SendAsync("worker-secret", new SecretRequest(args[1], args[2])).GetAwaiter().GetResult();
            DirectInput.Type(result.GetProperty("value").GetString()!, targetWindow);
            Print(AppPaths.Element(new { typed = true, field = args[2], clipboardUsed = false })); return 0;
        }
        if (args[0] == "exec" && args.Length > 1) {
            _ = CliRequest.Parse(args[1..]);
            var state = PipeTransport.SendAsync("status").GetAwaiter().GetResult();
            if (!state.GetProperty("unlocked").GetBoolean() && !UnlockFlow.RunDialog()) return 1;
            Print(PipeTransport.SendAsync("execute", args[1..]).GetAwaiter().GetResult()); return 0;
        }
        throw new ArgumentException("Commandes : status, unlock, lock, catalog [recherche], type <fiche> <champ>, exec <commande Bitwarden>.");
    }
    private static void Print(JsonElement result)
    {
        if (result.ValueKind == JsonValueKind.Object && result.TryGetProperty("object", out var kind)) {
            if (kind.GetString() is "string" or "list" && result.TryGetProperty("data", out var data)) result = data;
            else if (kind.GetString() == "template" && result.TryGetProperty("template", out var template)) result = template;
        }
        Console.WriteLine(result.ValueKind == JsonValueKind.String ? result.GetString() : result.GetRawText());
    }
}
