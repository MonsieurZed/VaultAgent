using System.Diagnostics;
using System.Text.Json;

namespace Homelab.Vault;

internal sealed class TrayController : ApplicationContext
{
    private readonly NotifyIcon icon = new();
    private readonly System.Windows.Forms.Timer timer = new() { Interval = 5000 };
    private readonly ToolStripMenuItem status = new(UiText.T("Coffre verrouillé")) { Enabled = false };
    private readonly Icon locked = ShieldIcon.Create(), unlocked = ShieldIcon.Create(unlocked: true);
    private bool polling;
    public TrayController()
    {
        icon.Icon = locked; icon.Text = UiText.T("Coffre Bitwarden");
        var menu = new ContextMenuStrip(); menu.Items.Add(status);
        var open = menu.Items.Add(UiText.T("Déverrouiller")); open.Click += async (_, _) => { try { UnlockFlow.RunDialog(); await RefreshAsync(); } catch (Exception error) { ShowError(error.Message); } };
        var configure = menu.Items.Add(UiText.T("Compte et sécurité…")); configure.Click += async (_, _) => await SettingsAsync();
        var passkeys = menu.Items.Add(UiText.T("Passkeys Windows…")); passkeys.Click += (_, _) => { try { Process.Start(new ProcessStartInfo(Path.Combine(AppPaths.Install, "VaultAgent.exe")) { UseShellExecute = false, ArgumentList = { "--passkeys-settings" } }); } catch (Exception error) { ShowError(error.Message); } };
        var lockItem = menu.Items.Add(UiText.T("Verrouiller maintenant")); lockItem.Click += async (_, _) => { try { await PipeTransport.SendAsync("lock"); await RefreshAsync(); } catch (Exception error) { ShowError(error.Message); } };
        menu.Items.Add(new ToolStripSeparator());
        var preferences = menu.Items.Add(UiText.T("Langue et mises à jour…"));
        preferences.Click += async (_, _) => {
            var previous = UserPreferences.Language;
            using var form = new ApplicationSettingsForm();
            if (form.ShowDialog() == DialogResult.OK) {
                foreach (var item in menu.Items.OfType<ToolStripMenuItem>()) {
                    var source = UiText.Entries.Keys.FirstOrDefault(key => UiText.Translate(key, previous) == item.Text);
                    if (source != null) item.Text = UiText.T(source);
                }
                await RefreshAsync();
            }
        };
        var quit = menu.Items.Add(UiText.T("Quitter et verrouiller")); quit.Click += async (_, _) => { try { await PipeTransport.SendAsync("lock"); } catch { } ExitThread(); };
        icon.ContextMenuStrip = menu; icon.Visible = true;
        icon.DoubleClick += async (_, _) => await SettingsAsync();
        timer.Tick += async (_, _) => await RefreshAsync(); timer.Start();
        _ = RefreshAsync();
    }
    private static void ShowError(string message) => MessageBox.Show(message, UiText.T("Coffre Bitwarden"), MessageBoxButtons.OK, MessageBoxIcon.Error);
    private static async Task SettingsAsync() { try { await UnlockFlow.ElevateAsync("--settings", "--owner", AppPaths.Sid); } catch (Exception error) { ShowError(error.Message); } }
    private async Task RefreshAsync()
    {
        if (polling) return; polling = true;
        try {
            var state = await PipeTransport.SendAsync("status");
            var opened = state.GetProperty("unlocked").GetBoolean();
            var expiration = state.GetProperty("expiresUtc");
            var text = opened ? expiration.ValueKind == JsonValueKind.Null ? UiText.T("Déverrouillé · délai désactivé") : UiText.T("Déverrouillé · ") + Math.Max(0, Math.Ceiling((expiration.GetDateTimeOffset() - DateTimeOffset.UtcNow).TotalMinutes)) + " min" : UiText.T("Coffre verrouillé");
            status.Text = text; icon.Text = text; icon.Icon = opened ? unlocked : locked;
        }
        catch { status.Text = UiText.T("Service du coffre indisponible"); icon.Text = status.Text; icon.Icon = locked; }
        finally { polling = false; }
    }
    protected override void Dispose(bool disposing)
    {
        if (disposing) { timer.Stop(); timer.Dispose(); icon.Visible = false; icon.Dispose(); locked.Dispose(); unlocked.Dispose(); }
        base.Dispose(disposing);
    }
}
