using System.Diagnostics;

namespace Homelab.Vault;

internal sealed class ApplicationSettingsForm : Form
{
    private readonly ComboBox language = new() { DropDownStyle = ComboBoxStyle.DropDownList, Width = 310 };
    private readonly ComboBox channel = new() { DropDownStyle = ComboBoxStyle.DropDownList, Width = 310 };
    private readonly Label result = new() { AutoSize = false, Width = 520, Height = 75 };
    private readonly Button install = new() { AutoSize = true, Visible = false };
    private readonly CancellationTokenSource stop = new();
    private ReleaseCandidate? candidate;
    private bool disposed;
    public ApplicationSettingsForm()
    {
        Text = "VaultAgent · " + UiText.T("Application"); Icon = ShieldIcon.Create(); Font = new Font("Segoe UI", 10);
        ClientSize = new Size(640, 650); MinimumSize = new Size(640, 650); BackColor = Color.FromArgb(248, 247, 252); StartPosition = FormStartPosition.CenterScreen;
        var panel = new FlowLayoutPanel { Dock = DockStyle.Fill, Padding = new Padding(28), FlowDirection = FlowDirection.TopDown, WrapContents = false, AutoScroll = true };
        void Label(string text, bool heading = false) => panel.Controls.Add(new Label { Text = UiText.T(text), Width = 520, Height = heading ? 32 : 47, ForeColor = heading ? Color.FromArgb(66, 41, 108) : Color.DimGray, Font = new Font("Segoe UI", heading ? 15 : 10, heading ? FontStyle.Bold : FontStyle.Regular) });
        Label("Langue", true); language.Items.AddRange(new object[] { "Français", "English", "Español" }); language.SelectedIndex = (UiText.PreviewLanguage ?? UserPreferences.Language) switch { "fr" => 0, "es" => 2, _ => 1 }; panel.Controls.Add(language);
        Label("La langue est appliquée aux nouvelles fenêtres.");
        Label("Mises à jour", true); channel.Items.AddRange(new object[] { UiText.T("Stable"), UiText.T("Expérimental") }); channel.SelectedIndex = UserPreferences.Channel == "experimental" ? 1 : 0; panel.Controls.Add(channel);
        Label("Stable : versions validées. Expérimental : inclut les préversions et fonctions en test.");
        var save = new Button { Text = UiText.T("Enregistrer les préférences"), AutoSize = true, BackColor = Color.FromArgb(121, 72, 217), ForeColor = Color.White, FlatStyle = FlatStyle.Flat };
        save.Click += (_, _) => { SavePreferences(); DialogResult = DialogResult.OK; Close(); }; panel.Controls.Add(save);
        var check = new Button { Text = UiText.T("Rechercher une mise à jour"), AutoSize = true, Margin = new Padding(0, 22, 0, 6) }; panel.Controls.Add(check);
        result.Text = UiText.T("Version installée : ") + UpdateService.Current.ToString(3); panel.Controls.Add(result); panel.Controls.Add(install);
        check.Click += async (_, _) => {
            check.Enabled = install.Enabled = false; install.Visible = false; result.Text = UiText.T("Recherche en cours…");
            try {
                candidate = await UpdateService.CheckAsync(channel.SelectedIndex == 1 ? "experimental" : "stable", stop.Token);
                result.Text = UiText.T("Version installée : ") + UpdateService.Current.ToString(3) + "\n" + (candidate == null ? UiText.T("Aucune version disponible sur ce canal.") : UiText.T("Version proposée : ") + candidate.Tag);
                install.Visible = candidate != null && candidate.Version > new Version(UpdateService.Current.ToString(3));
                install.Text = UiText.T("Télécharger et ouvrir l’installateur");
                if (candidate != null && !install.Visible) result.Text += "\n" + UiText.T("Aucune version plus récente. Aucun retour automatique à une ancienne version.");
            }
            catch (OperationCanceledException) { if (!IsDisposed) result.Text = UiText.T("Recherche annulée ou délai dépassé."); }
            catch { result.Text = UiText.T("Impossible de contacter GitHub. Réessayer plus tard."); }
            finally { if (!IsDisposed) check.Enabled = install.Enabled = true; }
        };
        channel.SelectedIndexChanged += (_, _) => { candidate = null; install.Visible = false; };
        install.Click += async (_, _) => {
            if (candidate == null) return; install.Enabled = check.Enabled = channel.Enabled = false; result.Text = UiText.T("Téléchargement et vérification SHA256…");
            try { var path = await UpdateService.DownloadAsync(candidate, stop.Token); Process.Start(new ProcessStartInfo(path) { UseShellExecute = true }); result.Text = UiText.T("Valider l’installation et l’UAC dans la fenêtre Windows."); }
            catch (OperationCanceledException) { if (!IsDisposed) result.Text = UiText.T("Recherche annulée ou délai dépassé."); }
            catch { result.Text = UiText.T("Téléchargement refusé ou vérification échouée."); }
            finally { if (!IsDisposed) install.Enabled = check.Enabled = channel.Enabled = true; }
        };
        Controls.Add(panel); FormClosing += (_, _) => stop.Cancel();
    }
    private void SavePreferences() => UserPreferences.Save(language.SelectedIndex switch { 0 => "fr", 2 => "es", _ => "en" }, channel.SelectedIndex == 1 ? "experimental" : "stable");
    protected override void Dispose(bool disposing) { if (disposing && !disposed) { disposed = true; stop.Cancel(); stop.Dispose(); } base.Dispose(disposing); }
}
