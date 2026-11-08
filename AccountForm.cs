using System.Text.Json;

namespace Homelab.Vault;

internal sealed partial class AccountForm : Form
{
    private readonly TextBox email = new(), server = new(), password = new(), code = new(), certificate = new();
    private readonly ComboBox mode = new() { DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly CheckBox automatic = new() { Text = UiText.T("Verrouillage automatique"), AutoSize = true, Checked = true };
    private readonly CheckBox blind = new() { Text = UiText.T("Les agents remplissent les champs sans lire les secrets"), AutoSize = true, Checked = true };
    private readonly NumericUpDown minutes = new() { Minimum = 1, Maximum = 1440, Value = 15 };
    private readonly Label state = new() { AutoSize = true, ForeColor = Color.DimGray };
    private readonly Button save = new() { Text = UiText.T("Vérifier et enregistrer"), AutoSize = true };
    private readonly Button forget = new() { Text = UiText.T("Oublier le compte"), AutoSize = true, Enabled = false };
    private HelloCredential? hello;
    private async Task LoadAsync()
    {
        try {
            if (!UnlockFlow.Elevated) { save.Text = UiText.T("Modifier avec l'UAC"); state.Text = UiText.T("La modification du compte demande l'UAC."); return; }
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
            state.Text = HelloNative.Available ? UiText.T("Windows Hello disponible : PIN, visage ou empreinte selon Windows.") : UiText.T("Windows Hello indisponible ; le mode UAC reste disponible.");
        }
        catch (Exception error) { state.Text = UiText.T(error.Message); }
    }
    private async Task SaveAsync()
    {
        if (!UnlockFlow.Elevated) { await UnlockFlow.ElevateAsync("--settings", "--owner", AppPaths.Sid); Close(); return; }
        save.Enabled = false; state.Text = UiText.T("Vérification du compte…");
        try {
            if (mode.SelectedIndex != 0 && hello == null) {
                if (!HelloNative.Available) throw new InvalidOperationException(UiText.T("Activer Windows Hello dans les paramètres Windows ou choisir l'UAC."));
                var window = Handle; var address = email.Text;
                hello = await Task.Run(() => HelloNative.Register(window, AppPaths.Sid, address));
            }
            var pem = string.IsNullOrWhiteSpace(certificate.Text) ? null : await File.ReadAllTextAsync(certificate.Text);
            var value = new ConfigureAccount(email.Text, server.Text, mode.SelectedIndex switch { 1 => "hello", 2 => "hello-or-uac", _ => "uac" },
                automatic.Checked ? (int)minutes.Value : null, password.Text, code.Text, hello, pem, blind.Checked);
            await PipeTransport.SendAsync("configure", value);
            password.Clear(); code.Clear();
            forget.Enabled = true;
            state.Text = UiText.T("Compte vérifié et enregistré. Le coffre est déverrouillé.");
        }
        catch (Exception error) { state.Text = UiText.T(error.Message); MessageBox.Show(this, error.Message, UiText.T("Configuration du coffre"), MessageBoxButtons.OK, MessageBoxIcon.Error); }
        finally { save.Enabled = true; }
    }
}
