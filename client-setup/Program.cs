namespace Homelab.Vault.ClientSetup;
internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        if (args.Contains("--verify-package")) { try { Package.Verify(); return 0; } catch { return 1; } }
        if (args.Contains("--install")) {
            try { Install.Run(desktop: true, startup: true); Console.WriteLine("{\"installed\":true,\"startup\":true}"); return 0; }
            catch { Console.Error.WriteLine("Installation du client interrompue."); return 1; }
        }
        ApplicationConfiguration.Initialize(); Application.Run(new SetupForm()); return 0;
    }
}
