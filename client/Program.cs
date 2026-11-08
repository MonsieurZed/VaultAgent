internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        try {
            return Homelab.Vault.Remote.RemoteCommands.Run(args.Length == 0 ? new[] { "settings" } : args);
        } catch (Exception error) {
            Console.Error.WriteLine(error is FileNotFoundException or DirectoryNotFoundException ? "Client not paired: open VaultAgent-Client.exe and enter a pairing code." : "Remote request refused. Check pairing and server authorization.");
            return 1;
        }
    }
}
