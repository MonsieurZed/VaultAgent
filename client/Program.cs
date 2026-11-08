internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        try {
            return Homelab.Vault.Remote.RemoteCommands.Run(args.Length == 0 ? new[] { "settings" } : args);
        } catch (Exception error) {
            if (error is Homelab.Vault.Remote.RemoteRequestException remote) { Console.Error.WriteLine(remote.Code); return 1; }
            Console.Error.WriteLine(error is FileNotFoundException or DirectoryNotFoundException ? "Client not paired: open VaultAgent-Client.exe and enter a pairing code." : "Remote request refused. Check pairing and server authorization.");
            return 1;
        }
    }
}
