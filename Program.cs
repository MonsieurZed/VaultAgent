using System.ServiceProcess;
using System.Text.Json;

namespace Homelab.Vault;

internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        try {
            if (args.Length > 0 && args[0] == "--remote") return Remote.RemoteCommands.Run(args.Length == 1 ? new[] { "settings" } : args[1..]);
            if (args.Length == 2 && args[0] == "--check-updates") { Print(AppPaths.Element(UpdateService.CheckAsync(args[1], CancellationToken.None).GetAwaiter().GetResult() is {} release ? new { release.Tag, version = release.Version.ToString(3), release.Experimental } : new { Tag = "", version = "", Experimental = false })); return 0; }
            if (args.Contains("--passkeys-status")) { Print(AppPaths.Element(Passkeys.WindowsPasskeyApi.Status())); return 0; }
            if (args.Contains("--service")) { ServiceBase.Run(new BrokerService()); return 0; }
            if (args.Contains("--self-test")) { SelfTests.Run(); return 0; }
            if (args.Contains("--engine-smoke")) { EngineSmoke.Run(); return 0; }
            if (args.Contains("--capabilities")) { Print(AppPaths.Element(new { helloAvailable = HelloNative.Available, webauthnApiVersion = HelloNative.ApiVersion, directInput = true, directInputRequiresWindowIdentity = true, browserAdapter = true, vaultPasskeyProvider = true, passkeysExperimental = true })); return 0; }
            ApplicationConfiguration.Initialize();
            if (args.Contains("--write-passkey-assets")) { Passkeys.ProviderActivation.WriteAssets(args[^1]); return 0; }
            if (args.Contains("--passkeys-settings")) {
                if (!Passkeys.WindowsPasskeyApi.HasOwnIdentity && Passkeys.ProviderActivation.IsInstalled) Passkeys.ProviderActivation.Launch("--passkeys-settings");
                else Application.Run(new Passkeys.PasskeySettingsForm());
                return 0;
            }
            if (args.Contains("--passkey-provider")) { Passkeys.ProviderHost.Run(); return 0; }
            if (args.Contains("--passkeys-register")) { Passkeys.ProviderRegistration.RegisterAsync().GetAwaiter().GetResult(); MessageBox.Show("Fournisseur enregistré. Activer VaultAgent dans les options avancées des passkeys Windows.", "VaultAgent"); return 0; }
            if (args.Contains("--passkeys-sync")) { Passkeys.ProviderSynchronization.Run(); return 0; }
            if (args.Contains("--passkeys-unregister")) { if (Passkeys.WindowsPasskeyApi.State().Registered) Passkeys.WindowsPasskeyApi.Check(Passkeys.WindowsPasskeyApi.WebAuthNPluginRemoveAuthenticator(Passkeys.WindowsPasskeyApi.ClassId)); return 0; }
            if (args.Contains("--write-icons")) { ShieldIcon.WriteAssets(args[^1]); return 0; }
            if (args.Length is >= 2 and <= 4 && args[0] == "--render-ui") { UiText.PreviewLanguage = args.Length >= 3 ? args[2] : "fr"; UiTests.Render(args[1], args.Length == 4 ? int.Parse(args[3]) : 0); return 0; }
            if (args.Contains("--application-settings")) { Application.Run(new ApplicationSettingsForm()); return 0; }
            if (args.Length == 3 && args[0] == "--render-preferences") { UiTests.RenderPreferences(args[1], args[2]); return 0; }
            if (args.Length == 3 && args[0] == "--render-remote") { UiTests.RenderRemote(args[1], args[2]); return 0; }
            if (args.Length == 2 && args[0] == "--render-passkeys-ui") { UiTests.RenderPasskeys(args[1]); return 0; }
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
            if (args.Length > 0 && args[0] == "--remote") { Console.Error.WriteLine(error is FileNotFoundException or DirectoryNotFoundException ? "Client distant non associe. Ouvrir VaultAgent-Client.exe et saisir un code d association." : "Client distant : operation refusee, connexion absente ou coffre verrouille."); return 1; }
            if (args.Length > 0 && args[0].StartsWith("--render-", StringComparison.Ordinal)) { Console.Error.WriteLine(error.ToString()); return 1; }
            if (args.Contains("--service")) return 1;
            if (args.Contains("--self-test") || args.Contains("--engine-smoke")) { Console.Error.WriteLine(error.Message); return 1; }
            if (args.Length > 0 && args[0] is "status" or "lock" or "unlock" or "exec" or "catalog" or "foreground" or "type" or "seal" or "seal-probe") { Console.Error.WriteLine(error.Message); return 1; }
            ApplicationConfiguration.Initialize();
            MessageBox.Show(error.Message, UiText.T("Coffre Bitwarden"), MessageBoxButtons.OK, MessageBoxIcon.Error); return 1;
        }
    }
    private static int RunCli(string[] args)
    {
        if (args[0] == "status") { Print(PipeTransport.SendAsync("status").GetAwaiter().GetResult()); return 0; }
        if (args[0] == "lock") { PipeTransport.SendAsync("lock").GetAwaiter().GetResult(); Console.WriteLine("Verrouillé."); return 0; }
        if (args[0] == "unlock") return UnlockFlow.RunDialog() ? 0 : 1;
        if (args[0] == "seal-probe" && args.Length == 2) { Print(AppPaths.Element(SealedSecret.Encrypt("vault-agent-paste-probe-20261107", args[1]))); return 0; }
        if (args[0] == "foreground") return TypeCommand.Foreground();
        if (args[0] == "type") return TypeCommand.Run(args[1..]);
        if (args[0] is "catalog" or "seal") {
            var status = PipeTransport.SendAsync("status").GetAwaiter().GetResult();
            if (!status.GetProperty("unlocked").GetBoolean() && !UnlockFlow.RunDialog()) return 1;
            if (args[0] == "catalog") { Print(PipeTransport.SendAsync("catalog", new { search = args.Length > 1 ? args[1] : "" }).GetAwaiter().GetResult()); return 0; }
            if (args[0] == "seal" && args.Length == 4) {
                Print(PipeTransport.SendAsync("seal", new { request = new SecretRequest(args[1], args[2]), publicKey = args[3] }).GetAwaiter().GetResult()); return 0;
            }
            throw new ArgumentException("Usage : seal <id-fiche> <champ> <clé-publique>.");
        }
        if (args[0] == "exec" && args.Length > 1) {
            _ = CliRequest.Parse(args[1..]);
            var state = PipeTransport.SendAsync("status").GetAwaiter().GetResult();
            if (!state.GetProperty("unlocked").GetBoolean() && !UnlockFlow.RunDialog()) return 1;
            Print(PipeTransport.SendAsync("execute", args[1..]).GetAwaiter().GetResult()); return 0;
        }
        throw new ArgumentException("Commandes : status, unlock, lock, catalog [recherche], foreground, type <fiche> <champ> --process <exe> --title <fragment>, exec <commande Bitwarden>.");
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
