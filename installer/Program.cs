using System.Security.Principal;

namespace Homelab.Vault.Setup;

internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        if (args.Contains("--verify-package")) {
            try { Payload.Verify(); return 0; } catch { return 1; }
        }
        if (args.Contains("--install-worker")) {
            using var mutex = new Mutex(false, "Global\\HomelabVaultAgentSetup");
            var owns = false;
            try {
                try { owns = mutex.WaitOne(0); } catch (AbandonedMutexException) { owns = true; }
                if (!owns) throw new IOException("Une installation de VaultAgent est déjà en cours.");
                var owner = Array.IndexOf(args, "--owner");
                if (!new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator) ||
                    owner < 0 || owner + 1 >= args.Length || args[owner + 1] != WindowsIdentity.GetCurrent().User!.Value)
                    throw new UnauthorizedAccessException("Valider l'UAC avec le même compte Windows.");
                SetupOperations.Install(); return 0;
            }
            catch (Exception error) { SetupOperations.WriteResult(false, error.Message); return 1; }
            finally { if (owns) mutex.ReleaseMutex(); }
        }
        ApplicationConfiguration.Initialize();
        using var form = new SetupForm();
        if (args.Length == 2 && args[0] == "--render-ui") {
            using var timer = new System.Windows.Forms.Timer { Interval = 300 };
            timer.Tick += (_, _) => { timer.Stop(); using var bitmap = new Bitmap(form.Width, form.Height); form.DrawToBitmap(bitmap, new Rectangle(Point.Empty, bitmap.Size)); bitmap.Save(args[1]); form.Close(); };
            form.Shown += (_, _) => timer.Start(); Application.Run(form); return 0;
        }
        Application.Run(form); return 0;
    }
}
