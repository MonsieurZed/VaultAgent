using System.Diagnostics;
using System.Text.Json;

namespace Homelab.Vault;

internal sealed class TrayController : ApplicationContext
{
    private readonly NotifyIcon icon = new();
    private readonly System.Windows.Forms.Timer timer = new() { Interval = 5000 };
    private readonly ToolStripMenuItem status = new("Coffre verrouillé") { Enabled = false };
    private readonly Icon locked = ShieldIcon.Create(), unlocked = ShieldIcon.Create(unlocked: true);
    private bool polling;
    public TrayController()
    {
        icon.Icon = locked; icon.Text = "Coffre Bitwarden";
        var menu = new ContextMenuStrip(); menu.Items.Add(status);
        var open = menu.Items.Add("Déverrouiller"); open.Click += async (_, _) => { try { UnlockFlow.RunDialog(); await RefreshAsync(); } catch (Exception error) { ShowError(error.Message); } };
        var configure = menu.Items.Add("Compte et sécurité…"); configure.Click += async (_, _) => await SettingsAsync();
        var lockItem = menu.Items.Add("Verrouiller maintenant"); lockItem.Click += async (_, _) => { try { await PipeTransport.SendAsync("lock"); await RefreshAsync(); } catch (Exception error) { ShowError(error.Message); } };
        menu.Items.Add(new ToolStripSeparator());
        var quit = menu.Items.Add("Quitter et verrouiller"); quit.Click += async (_, _) => { try { await PipeTransport.SendAsync("lock"); } catch { } ExitThread(); };
        icon.ContextMenuStrip = menu; icon.Visible = true;
        icon.DoubleClick += async (_, _) => await SettingsAsync();
        timer.Tick += async (_, _) => await RefreshAsync(); timer.Start();
        _ = RefreshAsync();
    }
    private static void ShowError(string message) => MessageBox.Show(message, "Coffre Bitwarden", MessageBoxButtons.OK, MessageBoxIcon.Error);
    private static async Task SettingsAsync() { try { await UnlockFlow.ElevateAsync("--settings", "--owner", AppPaths.Sid); } catch (Exception error) { ShowError(error.Message); } }
    private async Task RefreshAsync()
    {
        if (polling) return; polling = true;
        try {
            var state = await PipeTransport.SendAsync("status");
            var opened = state.GetProperty("unlocked").GetBoolean();
            var expiration = state.GetProperty("expiresUtc");
            var text = opened ? expiration.ValueKind == JsonValueKind.Null ? "Déverrouillé · délai désactivé" : "Déverrouillé · " + Math.Max(0, Math.Ceiling((expiration.GetDateTimeOffset() - DateTimeOffset.UtcNow).TotalMinutes)) + " min" : "Coffre verrouillé";
            status.Text = text; icon.Text = text; icon.Icon = opened ? unlocked : locked;
        }
        catch { status.Text = "Service du coffre indisponible"; icon.Text = status.Text; icon.Icon = locked; }
        finally { polling = false; }
    }
    protected override void Dispose(bool disposing)
    {
        if (disposing) { timer.Stop(); timer.Dispose(); icon.Visible = false; icon.Dispose(); locked.Dispose(); unlocked.Dispose(); }
        base.Dispose(disposing);
    }
}
