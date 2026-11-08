namespace Homelab.Vault.Remote;

internal sealed class RemoteClientForm : Form
{
    public RemoteClientForm(bool preview = false, string? initialServer = null)
    {
        Text = "VaultAgent · Client Windows"; Icon = ShieldIcon.Create(); Font = new Font("Segoe UI", 10); ClientSize = new Size(650, 490); Padding = new Padding(26); StartPosition = FormStartPosition.CenterScreen; BackColor = Color.FromArgb(248, 247, 252);
        var flow = new FlowLayoutPanel { Dock = DockStyle.Fill, FlowDirection = FlowDirection.TopDown, WrapContents = false };
        var title = new Label { AutoSize = true, Font = new Font("Segoe UI Semibold", 19), ForeColor = Color.FromArgb(99, 53, 174) };
        var info = new Label { Width = 585, Height = 65 };
        var server = new TextBox { Width = 580, Text = "" };
        var code = new TextBox { Width = 580, UseSystemPasswordChar = true };
        var connect = new Button { AutoSize = true, BackColor = Color.FromArgb(121, 72, 217), ForeColor = Color.White, FlatStyle = FlatStyle.Flat, Margin = new Padding(0, 18, 0, 10) };
        var status = new Label { Width = 560, Height = 75, ForeColor = Color.DimGray };
        var addressLabel = new Label { AutoSize = true }; var codeLabel = new Label { AutoSize = true, Margin = new Padding(0, 16, 0, 3) };
        var labels = new (Control Control, string Source)[] { (title, "Connecter ce PC au serveur"), (info, "Aucun service Windows requis. Créez un code d’association dans l’interface du serveur, onglet Clients. Ne saisissez pas le mot de passe du coffre ici."), (addressLabel, "Adresse HTTPS du serveur VaultAgent"), (codeLabel, "Code d’association"), (connect, "Associer ce client") };
        void Translate() { foreach (var item in labels) item.Control.Text = UiText.T(item.Source); code.PlaceholderText = UiText.T("Code à usage unique, valable cinq minutes"); server.PlaceholderText = UiText.T("Exemple : https://vaultagent.example.com"); }
        var languages = new ComboBox { Width = 160, DropDownStyle = ComboBoxStyle.DropDownList, Margin = new Padding(0, 6, 0, 12) }; languages.Items.AddRange(new object[] { "Français", "English", "Español" }); languages.SelectedIndex = (UiText.PreviewLanguage ?? UserPreferences.Language) switch { "fr" => 0, "es" => 2, _ => 1 };
        languages.SelectedIndexChanged += (_, _) => { UiText.PreviewLanguage = null; UserPreferences.Save(languages.SelectedIndex switch { 0 => "fr", 2 => "es", _ => "en" }, UserPreferences.Channel); Translate(); };
        foreach (var control in new Control[] { title, languages, info, addressLabel, server, codeLabel, code, connect, status }) flow.Controls.Add(control); Translate();
        connect.Click += async (_, _) => {
            connect.Enabled = false;
            try { await RemoteConnection.PairAsync(server.Text, code.Text.Trim()); code.Clear(); status.Text = UiText.T("Client associé. Le jeton est protégé par Windows pour votre compte. Autorisez les clients depuis le serveur pour remplir les champs."); }
            catch { status.Text = UiText.T("Association refusée. Vérifiez l’adresse HTTPS du serveur et le code, qui expire après cinq minutes."); }
            finally { connect.Enabled = true; }
        };
        Controls.Add(flow);
    }
}
