using System.Diagnostics;

namespace Homelab.Vault.Remote;

internal sealed class RemoteTray : ApplicationContext
{
    private readonly NotifyIcon icon = new() { Icon = ShieldIcon.Create(), Text = "VaultAgent Client", Visible = true };
    private readonly System.Windows.Forms.Timer timer = new() { Interval = 5000 };
    private readonly ToolStripMenuItem state = new() { Enabled = false };
    private bool busy;
    private readonly HashSet<string> notified = new();
    private System.Text.Json.JsonElement? lastRequest;
    private RemoteApprovalForm? approval;
    public RemoteTray()
    {
        var menu = new ContextMenuStrip(); menu.Items.Add(state);
        var portal = menu.Items.Add(UiText.T("Ouvrir le serveur VaultAgent")); portal.Click += (_, _) => { if (string.IsNullOrEmpty(RemoteConnection.ConfiguredServer)) Configure(); else Process.Start(new ProcessStartInfo(Server) { UseShellExecute = true }); };
        var settings = menu.Items.Add(UiText.T("Associer ce client")); settings.Click += (_, _) => Configure();
        var lockItem = menu.Items.Add(UiText.T("Verrouiller maintenant")); lockItem.Click += async (_, _) => { try { await ForVaults(id => RemoteConnection.CallAsync("lock", new { }, id)); await Refresh(); } catch { state.Text = UiText.T("Service du coffre indisponible"); } };
        menu.Items.Add(new ToolStripSeparator()); var exit = menu.Items.Add(UiText.T("Quitter")); exit.Click += (_, _) => ExitThread();
        icon.ContextMenuStrip = menu; icon.DoubleClick += (_, _) => Configure();
        var requests = new ToolStripMenuItem(UiText.T("Demande de déverrouillage"));
        requests.Click += (_, _) => OpenRequest(); menu.Items.Insert(1, requests);
        var reset = menu.Items.Add(UiText.T("Désactiver l'acceptation automatique"));
        reset.Click += async (_, _) => { try { await ForVaults(RemoteConnection.ResetAutomaticAsync); icon.ShowBalloonTip(5000, "VaultAgent", UiText.T("Acceptation automatique désactivée"), ToolTipIcon.Info); } catch { state.Text = UiText.T("Service du coffre indisponible"); } };
        timer.Tick += async (_, _) => await Refresh(); timer.Start(); _ = Refresh();
    }
    private static string Server => string.IsNullOrEmpty(RemoteConnection.ConfiguredServer) ? RemoteConnection.DefaultServer : RemoteConnection.ConfiguredServer;
    private static async Task ForVaults(Func<string, Task> action) { var vaults = await RemoteConnection.CallAsync("vaults", new { }); foreach (var vault in vaults.EnumerateArray()) await action(vault.GetProperty("id").GetString()!); }
    private void OpenRequest() {
        if (approval is not null) { approval.Activate(); return; }
        if (lastRequest is not { } request || request.GetProperty("expiresUtc").GetDateTimeOffset() <= DateTimeOffset.UtcNow) return;
        approval = new RemoteApprovalForm(request); approval.FormClosed += (_, _) => { approval = null; }; approval.Show();
    }
    private void Configure() { using var form = new RemoteClientForm(initialServer: Server); form.ShowDialog(); _ = Refresh(); }
    private async Task Refresh()
    {
        if (busy) return; busy = true;
        try {
            if (string.IsNullOrEmpty(RemoteConnection.ConfiguredServer)) { state.Text = UiText.T("Client à associer"); return; }
            var vaults = await RemoteConnection.CallAsync("vaults", new { });
            var requests = new List<System.Text.Json.JsonElement>(); var unlocked = 0; var total = 0;
            foreach (var vault in vaults.EnumerateArray()) {
                var status = await RemoteConnection.CallAsync("status", new { }, vault.GetProperty("id").GetString());
                ++total; if (status.GetProperty("unlocked").GetBoolean()) ++unlocked;
                if (status.TryGetProperty("approvalRequests", out var pending)) requests.AddRange(pending.EnumerateArray().Select(value => value.Clone()));
            }
            lastRequest = null;
            {
                var ids = requests.Select(request => request.GetProperty("requestId").GetString()!).ToHashSet();
                notified.IntersectWith(ids);
                foreach (var request in requests) {
                    lastRequest = request.Clone();
                    if (approval is null && notified.Add(request.GetProperty("requestId").GetString()!)) { OpenRequest(); break; }
                }
            }
            state.Text = (total > 1 ? $"{unlocked}/{total} · " : "") + UiText.T(unlocked > 0 ? "Coffre disponible pour les clients" : "Coffre verrouillé"); icon.Text = state.Text.Length > 63 ? state.Text[..63] : state.Text;
        } catch { state.Text = UiText.T("Service du coffre indisponible"); }
        finally { busy = false; }
    }
    protected override void Dispose(bool disposing) { if (disposing) { approval?.Close(); timer.Stop(); timer.Dispose(); icon.Visible = false; icon.Icon?.Dispose(); icon.Dispose(); } base.Dispose(disposing); }
}
