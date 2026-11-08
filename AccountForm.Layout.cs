namespace Homelab.Vault;

internal sealed partial class AccountForm
{
    public AccountForm(bool preview = false)
    {
        Text = "VaultAgent · " + UiText.T("Compte et sécurité"); Icon = ShieldIcon.Create(64); Font = new Font("Segoe UI", 10);
        ClientSize = new Size(820, 760); MinimumSize = new Size(740, 680); StartPosition = FormStartPosition.CenterScreen; BackColor = Color.FromArgb(248, 247, 252);
        var header = new Panel { Dock = DockStyle.Top, Height = 86, BackColor = Color.FromArgb(107, 57, 194) };
        header.Controls.Add(new PictureBox { Image = ShieldIcon.Render(52), Bounds = new Rectangle(25, 16, 52, 52), SizeMode = PictureBoxSizeMode.Zoom });
        header.Controls.Add(new Label { Text = UiText.T("Votre coffre, simplement"), ForeColor = Color.White, Font = new Font("Segoe UI Semibold", 21), AutoSize = true, Location = new Point(95, 12) });
        header.Controls.Add(new Label { Text = UiText.T("Connectez votre compte, puis choisissez comment l’ouvrir."), ForeColor = Color.FromArgb(236, 225, 255), AutoSize = true, Location = new Point(98, 53) });
        var tabs = new TabControl { Dock = DockStyle.Fill, Padding = new Point(20, 10) };
        FlowLayoutPanel Page(string name) {
            var page = new TabPage(UiText.T(name)) { BackColor = BackColor, Padding = new Padding(6) }; tabs.TabPages.Add(page);
            var flow = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoScroll = true, FlowDirection = FlowDirection.TopDown, WrapContents = false, Padding = new Padding(20, 10, 20, 20) }; page.Controls.Add(flow);
            flow.SizeChanged += (_, _) => { foreach (Control child in flow.Controls) child.Width = Math.Max(540, flow.ClientSize.Width - 58); }; return flow;
        }
        var account = Page("Compte"); var security = Page("Sécurité"); var application = Page("Application");
        void Field(FlowLayoutPanel flow, string label, Control control, string hint, int height = 104) {
            var card = new Panel { Width = 700, Height = height, BackColor = Color.White, Margin = new Padding(0, 0, 0, 12) };
            card.Controls.Add(new Label { Text = UiText.T(label), Font = new Font(Font, FontStyle.Bold), AutoSize = true, Location = new Point(14, 10) });
            control.SetBounds(14, 36, card.Width - 28, 28); control.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right; card.Controls.Add(control);
            card.Controls.Add(new Label { Text = UiText.T(hint), ForeColor = Color.DimGray, AutoSize = false, Bounds = new Rectangle(14, 69, card.Width - 28, height - 72), Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right }); flow.Controls.Add(card);
        }
        server.PlaceholderText = "https://vault.example.com"; password.UseSystemPasswordChar = true;
        password.PlaceholderText = UiText.T("Déjà enregistré ? Laissez vide pour le conserver."); code.PlaceholderText = UiText.T("Uniquement si la double authentification est activée");
        Field(account, "Adresse du coffre", server, "L’adresse HTTPS que vous ouvrez pour accéder à Bitwarden ou Vaultwarden.");
        Field(account, "Adresse email", email, "L’adresse du compte de votre coffre.");
        Field(account, "Mot de passe maître", password, "Le mot de passe qui déchiffre le coffre. Il peut être différent du mot de passe SSO.", 112);
        Field(account, "Code 2FA · facultatif", code, "Le code à six chiffres de votre application d’authentification, si demandé.", 112);
        mode.Items.AddRange(new object[] { UiText.T("UAC · confirmation administrateur"), UiText.T("Windows Hello · PIN, visage ou empreinte"), UiText.T("Windows Hello avec secours UAC") }); mode.SelectedIndex = 0;
        Field(security, "Comment ouvrir le coffre ?", mode, "UAC fonctionne sans configurer Hello. Les passkeys exigent toujours Hello.", 112);
        Field(security, "Quand verrouiller le coffre ?", automatic, "Après une période sans utilisation. Le verrouillage manuel reste toujours disponible.", 112);
        Field(security, "Délai en minutes", minutes, "15 minutes par défaut. Chaque utilisation autorisée prolonge le délai.");
        Field(security, "Accès des agents", blind, "Recommandé : l’agent peut remplir un champ sans recevoir sa valeur.", 112);
        var advanced = new Button { Text = UiText.T("Afficher les options avancées"), AutoSize = true }; account.Controls.Add(advanced);
        var advancedPanel = new FlowLayoutPanel { Width = 700, Height = 190, FlowDirection = FlowDirection.TopDown, Visible = false };
        Field(advancedPanel, "Certificat PEM · facultatif", certificate, "Laissez vide avec un certificat HTTPS reconnu. Réservé aux serveurs avec une autorité privée.", 120);
        var choose = new Button { Text = UiText.T("Choisir un certificat…"), AutoSize = true }; advancedPanel.Controls.Add(choose); account.Controls.Add(advancedPanel);
        advanced.Click += (_, _) => { advancedPanel.Visible = !advancedPanel.Visible; advanced.Text = UiText.T(advancedPanel.Visible ? "Masquer les options avancées" : "Afficher les options avancées"); };
        choose.Click += (_, _) => { using var dialog = new OpenFileDialog { Filter = "PEM|*.pem;*.crt|All|*.*" }; if (dialog.ShowDialog(this) == DialogResult.OK) certificate.Text = dialog.FileName; };
        var preferences = new Button { Text = UiText.T("Langue et mises à jour…"), AutoSize = true };
        Field(application, "Personnaliser VaultAgent", preferences, "Français, English, Español et choix du canal Stable ou Expérimental.", 112);
        preferences.Click += (_, _) => { using var form = new ApplicationSettingsForm(); form.ShowDialog(this); };
        var passkeys = new Button { Text = UiText.T("Configurer les passkeys Windows…"), AutoSize = true };
        Field(application, "Passkeys · expérimental", passkeys, "Pour créer et utiliser des passkeys. Windows Hello doit être configuré sur ce PC.", 112);
        passkeys.Click += (_, _) => System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(Path.Combine(AppPaths.Install, "VaultAgent.exe")) { UseShellExecute = false, ArgumentList = { "--passkeys-settings" } });
        var footer = new TableLayoutPanel { Dock = DockStyle.Bottom, Height = 92, Padding = new Padding(24, 12, 24, 12), ColumnCount = 3, BackColor = Color.FromArgb(240, 234, 250) };
        footer.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); footer.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); footer.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        state.AutoSize = false; state.Dock = DockStyle.Fill; save.BackColor = Color.FromArgb(121, 72, 217); save.ForeColor = Color.White; save.FlatStyle = FlatStyle.Flat; save.Height = 38; save.Anchor = forget.Anchor = AnchorStyles.Right;
        footer.Controls.Add(state, 0, 0); footer.Controls.Add(forget, 1, 0); footer.Controls.Add(save, 2, 0); Controls.Add(tabs); Controls.Add(footer); Controls.Add(header);
        automatic.CheckedChanged += (_, _) => minutes.Enabled = automatic.Checked; save.Click += async (_, _) => await SaveAsync();
        forget.Click += async (_, _) => {
            if (MessageBox.Show(this, UiText.T("Oublier le mot de passe enregistré sur ce PC ? Le coffre distant est conservé."), UiText.T("Oublier le compte"), MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
            try { await PipeTransport.SendAsync("forget"); password.Clear(); code.Clear(); email.Clear(); hello = null; forget.Enabled = false; state.Text = UiText.T("Compte oublié sur ce PC."); }
            catch (Exception error) { MessageBox.Show(this, UiText.T(error.Message), "VaultAgent", MessageBoxButtons.OK, MessageBoxIcon.Error); }
        };
        if (preview) { email.Text = "example@example.invalid"; state.Text = UiText.T("Aperçu · aucun compte réel utilisé."); }
        else Shown += async (_, _) => await LoadAsync();
    }
}
