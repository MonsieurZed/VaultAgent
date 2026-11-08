using System.Text.Json;

namespace Homelab.Vault;

internal sealed class AccountForm : Form
{
    private readonly TextBox email = new(), server = new(), password = new(), code = new(), certificate = new();
    private readonly ComboBox mode = new() { DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly CheckBox automatic = new() { Text = "Verrouillage automatique", AutoSize = true, Checked = true };
    private readonly CheckBox blind = new() { Text = "Les agents remplissent les champs sans lire les secrets", AutoSize = true, Checked = true };
    private readonly NumericUpDown minutes = new() { Minimum = 1, Maximum = 1440, Value = 15 };
    private readonly Label state = new() { AutoSize = true, ForeColor = Color.DimGray };
    private readonly Button save = new() { Text = "Vérifier et enregistrer", AutoSize = true };
    private readonly Button forget = new() { Text = "Oublier le compte", AutoSize = true, Enabled = false };
    private HelloCredential? hello;
    public AccountForm(bool preview = false)
    {
        Text = "Coffre Bitwarden · Compte et sécurité"; Icon = ShieldIcon.Create(64); Font = new Font("Segoe UI", 10);
        ClientSize = new Size(650, 680); MinimumSize = new Size(620, 680); StartPosition = FormStartPosition.CenterScreen;
        var header = new Panel { Dock = DockStyle.Top, Height = 85, BackColor = Color.FromArgb(107, 57, 194) };
        header.Controls.Add(new PictureBox { Image = ShieldIcon.Render(56), SizeMode = PictureBoxSizeMode.Zoom, Bounds = new Rectangle(20, 14, 56, 56) });
        header.Controls.Add(new Label { Text = "Votre coffre, avec Windows", ForeColor = Color.White, Font = new Font("Segoe UI Semibold", 18), AutoSize = true, Location = new Point(94, 17) });
        header.Controls.Add(new Label { Text = "Compte · Déverrouillage · Accès des agents", ForeColor = Color.FromArgb(232, 218, 255), AutoSize = true, Location = new Point(96, 52) });
        Controls.Add(header);
        var table = new TableLayoutPanel { Dock = DockStyle.Fill, Padding = new Padding(24, 12, 24, 20), ColumnCount = 2, AutoScroll = true };
        table.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 190)); table.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        table.RowStyles.Clear();
        void Row(string label, Control control, int height = 44) {
            var index = table.RowCount++; table.RowStyles.Add(new RowStyle(SizeType.Absolute, height));
            table.Controls.Add(new Label { Text = label, AutoSize = true, Anchor = AnchorStyles.Left }, 0, index);
            control.Anchor = AnchorStyles.Left | AnchorStyles.Right; table.Controls.Add(control, 1, index);
        }
        mode.Items.AddRange(new object[] { "UAC uniquement", "Windows Hello uniquement", "Windows Hello ou UAC" }); mode.SelectedIndex = 0;
        server.PlaceholderText = "https://votre-serveur"; password.UseSystemPasswordChar = true;
        password.PlaceholderText = "Laisser vide pour conserver le mot de passe"; code.PlaceholderText = "Code authentificateur facultatif";
        Row("Adresse du serveur", server); Row("Adresse email", email); Row("Mot de passe maître", password); Row("Code 2FA", code);
        Row("Déverrouillage", mode); Row("Après l'utilisation", automatic); Row("Délai en minutes", minutes);
        Row("Accès des agents", blind, 52);
        Row("Certificat PEM local", certificate); certificate.PlaceholderText = "Facultatif, pour votre serveur privé";
        var choose = new Button { Text = "Choisir un certificat…", AutoSize = true };
        choose.Click += (_, _) => { using var dialog = new OpenFileDialog { Filter = "Certificat PEM|*.pem;*.crt|Tous les fichiers|*.*" }; if (dialog.ShowDialog(this) == DialogResult.OK) certificate.Text = dialog.FileName; };
        Row("", choose);
        var passkeys = new Button { Text = "Options des passkeys Windows…", AutoSize = true };
        passkeys.Click += (_, _) => System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo("ms-settings:passkeys") { UseShellExecute = true });
        Row("Passkeys natives", passkeys);
        var footer = new TableLayoutPanel { Dock = DockStyle.Bottom, Height = 100, Padding = new Padding(24, 10, 24, 10), ColumnCount = 3, BackColor = Color.FromArgb(247, 243, 255) };
        footer.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); footer.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); footer.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        state.AutoSize = false; state.Dock = DockStyle.Fill;
        save.Anchor = AnchorStyles.Right; save.Height = 38; save.BackColor = Color.FromArgb(121, 72, 217); save.ForeColor = Color.White; save.FlatStyle = FlatStyle.Flat;
        forget.Anchor = AnchorStyles.Right; footer.Controls.Add(state, 0, 0); footer.Controls.Add(forget, 1, 0); footer.Controls.Add(save, 2, 0); Controls.Add(footer);
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 3, Padding = Padding.Empty, Margin = Padding.Empty };
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 85)); layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100)); layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 100));
        header.Dock = DockStyle.Fill; header.Margin = Padding.Empty; footer.Dock = DockStyle.Fill; footer.Margin = Padding.Empty; table.Margin = Padding.Empty;
        layout.Controls.Add(header, 0, 0); layout.Controls.Add(table, 0, 1); layout.Controls.Add(footer, 0, 2); Controls.Add(layout);
        automatic.CheckedChanged += (_, _) => minutes.Enabled = automatic.Checked;
        save.Click += async (_, _) => await SaveAsync();
        forget.Click += async (_, _) => {
            if (MessageBox.Show(this, "Oublier le mot de passe enregistré sur ce PC ? Le coffre distant est conservé.", "Oublier le compte", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
            try { await PipeTransport.SendAsync("forget"); password.Clear(); code.Clear(); email.Clear(); hello = null; forget.Enabled = false; state.Text = "Compte oublié sur ce PC."; }
            catch (Exception error) { MessageBox.Show(this, error.Message, "Coffre Bitwarden", MessageBoxButtons.OK, MessageBoxIcon.Error); }
        };
        if (preview) { email.Text = "exemple@domaine.fr"; state.Text = "Aperçu de configuration · aucun compte réel utilisé."; }
        else Shown += async (_, _) => await LoadAsync();
    }
    private async Task LoadAsync()
    {
        try {
            if (!UnlockFlow.Elevated) { save.Text = "Modifier avec l'UAC"; state.Text = "La modification du compte demande l'UAC."; return; }
            var settings = await PipeTransport.SendAsync("settings");
            if (settings.GetProperty("configured").GetBoolean()) {
                forget.Enabled = true;
                email.Text = settings.GetProperty("email").GetString(); server.Text = settings.GetProperty("server").GetString();
                mode.SelectedIndex = settings.GetProperty("mode").GetString() switch { "hello" => 1, "hello-or-uac" => 2, _ => 0 };
                automatic.Checked = settings.GetProperty("timeoutMinutes").ValueKind != JsonValueKind.Null;
                if (automatic.Checked) minutes.Value = settings.GetProperty("timeoutMinutes").GetInt32();
                blind.Checked = settings.GetProperty("blindMode").GetBoolean();
                hello = settings.GetProperty("hello").Deserialize<HelloCredential>(AppPaths.Json);
            }
            state.Text = HelloNative.Available ? "Windows Hello disponible : PIN, visage ou empreinte selon Windows." : "Windows Hello indisponible ; le mode UAC reste disponible.";
        }
        catch (Exception error) { state.Text = error.Message; }
    }
    private async Task SaveAsync()
    {
        if (!UnlockFlow.Elevated) { await UnlockFlow.ElevateAsync("--settings", "--owner", AppPaths.Sid); Close(); return; }
        save.Enabled = false; state.Text = "Vérification du compte…";
        try {
            if (mode.SelectedIndex != 0 && hello == null) {
                if (!HelloNative.Available) throw new InvalidOperationException("Activer Windows Hello dans les paramètres Windows ou choisir l'UAC.");
                var window = Handle; var address = email.Text;
                hello = await Task.Run(() => HelloNative.Register(window, AppPaths.Sid, address));
            }
            var pem = string.IsNullOrWhiteSpace(certificate.Text) ? null : await File.ReadAllTextAsync(certificate.Text);
            var value = new ConfigureAccount(email.Text, server.Text, mode.SelectedIndex switch { 1 => "hello", 2 => "hello-or-uac", _ => "uac" },
                automatic.Checked ? (int)minutes.Value : null, password.Text, code.Text, hello, pem, blind.Checked);
            await PipeTransport.SendAsync("configure", value);
            password.Clear(); code.Clear();
            forget.Enabled = true;
            state.Text = "Compte vérifié et enregistré. Le coffre est déverrouillé.";
        }
        catch (Exception error) { state.Text = error.Message; MessageBox.Show(this, error.Message, "Configuration du coffre", MessageBoxButtons.OK, MessageBoxIcon.Error); }
        finally { save.Enabled = true; }
    }
}
