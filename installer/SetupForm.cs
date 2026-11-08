using System.ComponentModel;
using System.Diagnostics;
using System.Security.Principal;
using System.Text.Json;

namespace Homelab.Vault.Setup;

internal sealed class SetupForm : Form
{
    private readonly Button install = new() { Text = UiText.T("Installer"), Width = 140, Height = 38 };
    private readonly Button close = new() { Text = UiText.T("Fermer"), Width = 100, Height = 38 };
    private readonly CheckBox desktop = new() { Text = UiText.T("Créer un raccourci sur le bureau"), Checked = true, AutoSize = true };
    private readonly CheckBox startup = new() { Text = UiText.T("Démarrer le bouclier à l'ouverture de session"), Checked = true, AutoSize = true };
    private readonly Label status = new() { Text = UiText.T("Prêt à installer. Une validation UAC sera demandée."), AutoSize = false, Height = 72, Dock = DockStyle.Fill };
    private readonly ProgressBar progress = new() { Dock = DockStyle.Top, Height = 8, Visible = false };
    private bool working;
    public SetupForm()
    {
        Text = UiText.T("Installation de VaultAgent"); Icon = ShieldIcon.Create(64); Font = new Font("Segoe UI", 10);
        ClientSize = new Size(620, 460); FormBorderStyle = FormBorderStyle.FixedDialog; MaximizeBox = false; StartPosition = FormStartPosition.CenterScreen;
        var header = new Panel { Dock = DockStyle.Top, Height = 100, BackColor = Color.FromArgb(107, 57, 194) };
        header.Controls.Add(new PictureBox { Image = ShieldIcon.Render(64), Bounds = new Rectangle(23, 18, 64, 64), SizeMode = PictureBoxSizeMode.Zoom });
        header.Controls.Add(new Label { Text = "VaultAgent", Font = new Font("Segoe UI Semibold", 23), ForeColor = Color.White, AutoSize = true, Location = new Point(110, 15) });
        header.Controls.Add(new Label { Text = UiText.T("Installation Windows · version ") + typeof(SetupForm).Assembly.GetName().Version!.ToString(3) + " · x64", ForeColor = Color.White, AutoSize = true, Location = new Point(112, 61) });
        var content = new TableLayoutPanel { Dock = DockStyle.Fill, Padding = new Padding(25, 18, 25, 12), RowCount = 5, ColumnCount = 1 };
        content.RowStyles.Add(new RowStyle(SizeType.Absolute, 55)); content.RowStyles.Add(new RowStyle(SizeType.Absolute, 34)); content.RowStyles.Add(new RowStyle(SizeType.Absolute, 34));
        content.RowStyles.Add(new RowStyle(SizeType.Absolute, 56)); content.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        content.Controls.Add(new Label { Text = UiText.T("Service local, moteur Bitwarden et bouclier violet.\nLe compte et les données existants sont conservés."), AutoSize = true }, 0, 0);
        content.Controls.Add(desktop, 0, 1); content.Controls.Add(startup, 0, 2);
        content.Controls.Add(new Label { Text = UiText.T("Destination : ") + SetupOperations.Root + UiText.T("\n.NET et Node sont inclus. Aucun mot de passe dans l'installateur."), AutoSize = true, ForeColor = Color.DimGray }, 0, 3);
        content.Controls.Add(status, 0, 4);
        var footer = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 65, FlowDirection = FlowDirection.RightToLeft, Padding = new Padding(18, 12, 18, 10), BackColor = Color.FromArgb(247, 243, 255) };
        install.BackColor = Color.FromArgb(121, 72, 217); install.ForeColor = Color.White; install.FlatStyle = FlatStyle.Flat;
        footer.Controls.Add(close); footer.Controls.Add(install);
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 3 };
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 100)); layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100)); layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 65));
        header.Dock = content.Dock = footer.Dock = DockStyle.Fill;
        layout.Controls.Add(header, 0, 0); layout.Controls.Add(content, 0, 1); layout.Controls.Add(footer, 0, 2); Controls.Add(layout); Controls.Add(progress);
        close.Click += (_, _) => Close(); install.Click += async (_, _) => await InstallAsync();
        FormClosing += (_, e) => { if (working) e.Cancel = true; };
    }
    private async Task InstallAsync()
    {
        if (install.Text == UiText.T("Configurer le compte")) {
            try { Process.Start(new ProcessStartInfo(SetupOperations.Executable) { UseShellExecute = true, Verb = "runas", Arguments = "--settings --owner " + WindowsIdentity.GetCurrent().User!.Value }); Close(); }
            catch (Win32Exception error) when (error.NativeErrorCode == 1223) { status.Text = UiText.T("Configuration annulée. Le bouclier reste disponible."); }
            return;
        }
        working = true; install.Enabled = close.Enabled = desktop.Enabled = startup.Enabled = false;
        progress.Visible = true; progress.Style = ProgressBarStyle.Marquee; status.Text = UiText.T("Validation du paquet, installation et démarrage du service…");
        try {
            var info = new ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = true, Verb = "runas", WindowStyle = ProcessWindowStyle.Hidden };
            info.ArgumentList.Add("--install-worker"); info.ArgumentList.Add("--owner"); info.ArgumentList.Add(WindowsIdentity.GetCurrent().User!.Value);
            using var worker = Process.Start(info) ?? throw new IOException(UiText.T("Installation non démarrée."));
            await worker.WaitForExitAsync();
            if (worker.ExitCode != 0) {
                var message = UiText.T("Installation interrompue. Le compte existant est conservé.");
                try { using var result = JsonDocument.Parse(await File.ReadAllTextAsync(Path.Combine(SetupOperations.Root, "setup-result.json"))); message = result.RootElement.GetProperty("message").GetString() ?? message; } catch { }
                throw new IOException(message);
            }
            SetupOperations.Shortcuts(desktop.Checked, startup.Checked);
            Process.Start(new ProcessStartInfo(SetupOperations.Executable) { UseShellExecute = true, Arguments = "--tray" });
            status.Text = UiText.T("Installation terminée. Le bouclier est disponible près de l'horloge.\nVotre compte existant est conservé ; le coffre redemande une validation Windows.");
            install.Text = UiText.T("Configurer le compte");
        }
        catch (Win32Exception error) when (error.NativeErrorCode == 1223) { status.Text = UiText.T("Validation UAC annulée. Vous pouvez relancer l'installation."); }
        catch (Exception error) { status.Text = UiText.T(error.Message); }
        finally { working = false; install.Enabled = close.Enabled = desktop.Enabled = startup.Enabled = true; progress.Visible = false; }
    }
}
