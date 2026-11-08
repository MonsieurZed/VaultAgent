using System.Diagnostics;
namespace Homelab.Vault.ClientSetup;
internal sealed class SetupForm : Form
{
    public SetupForm()
    {
        Text = "Installer VaultAgent Client"; Icon = ShieldIcon.Create(); Font = new Font("Segoe UI", 10); ClientSize = new Size(620, 450); Padding = new Padding(28); StartPosition = FormStartPosition.CenterScreen; BackColor = Color.FromArgb(248, 247, 252); FormBorderStyle = FormBorderStyle.FixedDialog; MaximizeBox = false;
        var flow = new FlowLayoutPanel { Dock = DockStyle.Fill, FlowDirection = FlowDirection.TopDown, WrapContents = false };
        var title = new Label { Text = "VaultAgent Client", Font = new Font("Segoe UI Semibold", 24), AutoSize = true, ForeColor = Color.FromArgb(107, 57, 194) };
        var description = new Label { Text = "Le bouclier demarre avec Windows et relie ce PC au serveur Linux.\nUne association suffit. Aucun mot de passe maitre sur ce PC.", Width = 555, Height = 60 };
        var desktop = new CheckBox { Text = "Creer un raccourci sur le bureau", Checked = true, AutoSize = true };
        var startup = new CheckBox { Text = "Demarrer en arriere-plan avec Windows", Checked = true, AutoSize = true };
        var destination = new Label { Text = "Installation pour votre compte :\n" + Install.Root, Width = 555, Height = 52, ForeColor = Color.DimGray, Margin = new Padding(0, 12, 0, 6) };
        var button = new Button { Text = "Installer", AutoSize = true, BackColor = Color.FromArgb(121, 72, 217), ForeColor = Color.White, FlatStyle = FlatStyle.Flat };
        var status = new Label { Width = 555, Height = 65, ForeColor = Color.DimGray }; var installed = false;
        foreach (var control in new Control[] { title, description, desktop, startup, destination, button, status }) flow.Controls.Add(control);
        button.Click += async (_, _) => {
            if (installed) { Process.Start(new ProcessStartInfo(Install.Exe) { UseShellExecute = true, ArgumentList = { "settings" } }); Close(); return; }
            button.Enabled = desktop.Enabled = startup.Enabled = false; status.Text = "Verification du paquet et installation…";
            try {
                var onDesktop = desktop.Checked; var onStartup = startup.Checked; await Task.Run(() => Install.Run(onDesktop, onStartup));
                Process.Start(new ProcessStartInfo(Install.Exe) { UseShellExecute = true, ArgumentList = { "tray" } });
                installed = true; button.Text = "Associer ce PC"; status.Text = "Installe. Le bouclier est disponible pres de l horloge.\nCliquez sur Associer ce PC pour indiquer votre serveur.";
            } catch (Exception error) { status.Text = error.Message; }
            finally { button.Enabled = true; }
        };
        Controls.Add(flow);
    }
}
