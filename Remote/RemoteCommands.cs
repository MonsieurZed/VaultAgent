namespace Homelab.Vault.Remote;

internal static class RemoteCommands
{
    public static int Run(string[] args)
    {
        switch (args[0]) {
            case "tray": ApplicationConfiguration.Initialize(); using (var mutex = new Mutex(true, "Local\\VaultAgentRemoteClient-" + System.Security.Principal.WindowsIdentity.GetCurrent().User!.Value, out var first)) {
                if (!first) return 0; try { Application.Run(new RemoteTray()); } finally { mutex.ReleaseMutex(); }
            } return 0;
            case "settings": ApplicationConfiguration.Initialize(); Application.Run(new RemoteClientForm(initialServer: args.Length > 1 ? args[1] : null)); return 0;
            case "status": Print(RemoteConnection.CallAsync("status", new { }).GetAwaiter().GetResult()); return 0;
            case "lock": Print(RemoteConnection.CallAsync("lock", new { }).GetAwaiter().GetResult()); return 0;
            case "catalog": Print(RemoteConnection.CallAsync("catalog", new { search = args.Length > 1 ? args[1] : "" }).GetAwaiter().GetResult()); return 0;
            case "seal" when args.Length == 5: Print(RemoteConnection.CallAsync("seal", new { itemId = args[1], field = args[2], publicKey = args[3], origin = args[4] }).GetAwaiter().GetResult()); return 0;
            case "type" when args.Length == 4: Print(RemoteConnection.TypeAsync(args[1], args[2], args[3]).GetAwaiter().GetResult()); return 0;
            default: throw new ArgumentException("Commandes : settings, status, catalog, lock, seal, type.");
        }
    }
    private static void Print(object value) => Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(value, AppPaths.Json));
}
