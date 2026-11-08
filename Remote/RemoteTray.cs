using System.Diagnostics;

namespace Homelab.Vault.Remote;

internal sealed class RemoteTray : ApplicationContext
{
    private readonly NotifyIcon icon = new() { Icon = ShieldIcon.Create(), Text = "VaultAgent Client", Visible = true };
    private readonly System.Windows.Forms.Timer timer = new() { Interval = 15000 };
    private readonly ToolStripMenuItem state = new() { Enabled = false };
    private bool busy;
    public RemoteTray()
    {
        var menu = new ContextMenuStrip(); menu.Items.Add(state);
        var portal = menu.Items.Add(UiText.T("Ouvrir le serveur VaultAgent")); portal.Click += (_, _) => { if (string.IsNullOrEmpty(RemoteConnection.ConfiguredServer)) Configure(); else Process.Start(new ProcessStartInfo(Server) { UseShellExecute = true }); };
        var settings = menu.Items.Add(UiText.T("Associer ce client")); settings.Click += (_, _) => Configure();
        var lockItem = menu.Items.Add(UiText.T("Verrouiller maintenant")); lockItem.Click += async (_, _) => { try { await RemoteConnection.CallAsync("lock", new { }); await Refresh(); } catch { state.Text = UiText.T("Service du coffre indisponible"); } };
        menu.Items.Add(new ToolStripSeparator()); var exit = menu.Items.Add(UiText.T("Quitter")); exit.Click += (_, _) => ExitThread();
        icon.ContextMenuStrip = menu; icon.DoubleClick += (_, _) => Configure();
        timer.Tick += async (_, _) => await Refresh(); timer.Start(); _ = Refresh();
    }
    private static string Server => string.IsNullOrEmpty(RemoteConnection.ConfiguredServer) ? RemoteConnection.DefaultServer : RemoteConnection.ConfiguredServer;
    private void Configure() { using var form = new RemoteClientForm(initialServer: Server); form.ShowDialog(); _ = Refresh(); }
    private async Task Refresh()
    {
        if (busy) return; busy = true;
        try {
            if (string.IsNullOrEmpty(RemoteConnection.ConfiguredServer)) { state.Text = UiText.T("Client à associer"); return; }
            var status = await RemoteConnection.CallAsync("status", new { });
            state.Text = UiText.T(status.GetProperty("unlocked").GetBoolean() ? "Coffre disponible pour les clients" : "Coffre verrouillé"); icon.Text = state.Text.Length > 63 ? state.Text[..63] : state.Text;
        } catch { state.Text = UiText.T("Service du coffre indisponible"); }
        finally { busy = false; }
    }
    protected override void Dispose(bool disposing) { if (disposing) { timer.Stop(); timer.Dispose(); icon.Visible = false; icon.Icon?.Dispose(); icon.Dispose(); } base.Dispose(disposing); }
}
