namespace Homelab.Vault.Remote;

internal static class RemoteCommands
{
    public static int Run(string[] args)
    {
        var vaultFlag = Array.IndexOf(args, "--vault");
        if (vaultFlag >= 0) {
            if (vaultFlag + 1 >= args.Length) throw new ArgumentException("Identifiant de coffre requis.");
            RemoteConnection.TargetVault = args[vaultFlag + 1]; args = args.Where((_, index) => index != vaultFlag && index != vaultFlag + 1).ToArray();
        }
        if (args.Length == 0) throw new ArgumentException("Commande requise.");
        switch (args[0]) {
            case "tray": ApplicationConfiguration.Initialize(); using (var mutex = new Mutex(true, "Local\\VaultAgentRemoteClient-" + System.Security.Principal.WindowsIdentity.GetCurrent().User!.Value, out var first)) {
                if (!first) return 0; try { Application.Run(new RemoteTray()); } finally { mutex.ReleaseMutex(); }
            } return 0;
            case "settings": ApplicationConfiguration.Initialize(); Application.Run(new RemoteClientForm(initialServer: args.Length > 1 ? args[1] : null)); return 0;
            case "status": Print(RemoteConnection.CallAsync("status", new { }).GetAwaiter().GetResult()); return 0;
            case "vaults": Print(RemoteConnection.CallAsync("vaults", new { }).GetAwaiter().GetResult()); return 0;
            case "sharing": Print(RemoteConnection.CallAsync("sharing", new { }).GetAwaiter().GetResult()); return 0;
            case "share" when args.Length == 1:
                using (var document = System.Text.Json.JsonDocument.Parse(ReadInput())) Print(RemoteConnection.CallAsync("share", document.RootElement).GetAwaiter().GetResult()); return 0;
            case "approval-public-key": Print(RemoteConnection.ApprovalIdentity()); return 0;
            case "request-unlock":
                Print(RemoteConnection.CallAsync("request-unlock", new { }).GetAwaiter().GetResult());
                System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(Environment.ProcessPath!, "tray") { UseShellExecute = true, WindowStyle = System.Diagnostics.ProcessWindowStyle.Hidden }); return 0;
            case "lock": Print(RemoteConnection.CallAsync("lock", new { }).GetAwaiter().GetResult()); return 0;
            case "catalog": Print(RemoteConnection.CallAsync("catalog", new { search = args.Length > 1 ? args[1] : "" }).GetAwaiter().GetResult()); return 0;
            case "write" when args.Length == 1:
                using (var document = System.Text.Json.JsonDocument.Parse(ReadInput())) Print(RemoteConnection.CallAsync("write", document.RootElement).GetAwaiter().GetResult()); return 0;
            case "write-preview" when args.Length == 2: Print(RemoteConnection.CallAsync("write-preview", new { itemId = args[1] }).GetAwaiter().GetResult()); return 0;
            case "write-preview" when args.Length == 1: Print(RemoteConnection.CallAsync("write-preview", new { }).GetAwaiter().GetResult()); return 0;
            case "seal" when args.Length == 5: Print(RemoteConnection.CallAsync("seal", new { itemId = args[1], field = args[2], publicKey = args[3], origin = args[4] }).GetAwaiter().GetResult()); return 0;
            case "type" when args.Length == 4: Print(RemoteConnection.TypeAsync(args[1], args[2], args[3]).GetAwaiter().GetResult()); return 0;
            default: throw new ArgumentException("Commandes : settings, vaults, status, request-unlock, catalog, lock, seal, type, write-preview, sharing, share, write (JSON stdin) ; --vault UUID.");
        }
    }
    private static void Print(object value) => Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(value, AppPaths.Json));
    private static string ReadInput() {
        if (!Console.IsInputRedirected) throw new ArgumentException("Flux redirige requis.");
        Console.InputEncoding = new System.Text.UTF8Encoding(false, true);
        var output = new System.Text.StringBuilder(); var block = new char[2048]; int count;
        while ((count = Console.In.Read(block, 0, block.Length)) != 0) { if (output.Length + count > 24000) throw new ArgumentException("Requete trop volumineuse."); output.Append(block, 0, count); }
        return output.ToString();
    }
}
