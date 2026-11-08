using System.Diagnostics;
using System.Security.Cryptography.X509Certificates;
using System.Text.Json;

namespace Homelab.Vault.Passkeys;

internal sealed class PasskeySettingsForm : Form
{
    private readonly Label state = new() { AutoSize = false, Dock = DockStyle.Top, Height = 110 };
    private readonly Label details = new() { AutoSize = false, Dock = DockStyle.Bottom, Height = 95, ForeColor = Color.DimGray };
    private readonly Button identity = new() { Text = UiText.T("Installer l'identité du fournisseur"), AutoSize = true };
    private readonly Button register = new() { Text = UiText.T("Enregistrer VaultAgent dans Windows"), AutoSize = true };
    public PasskeySettingsForm()
    {
        Text = UiText.T("VaultAgent · Passkeys Windows"); Icon = ShieldIcon.Create(64); Font = new Font("Segoe UI", 10); ClientSize = new Size(610, 510); Padding = new Padding(22); StartPosition = FormStartPosition.CenterScreen;
        var actions = new FlowLayoutPanel { Dock = DockStyle.Fill, FlowDirection = FlowDirection.TopDown, WrapContents = false, Padding = new Padding(0, 12, 0, 0) };
        var settings = new Button { Text = UiText.T("Activer le fournisseur dans les paramètres Windows…"), AutoSize = true };
        var hello = new Button { Text = UiText.T("Options de connexion Windows Hello…"), AutoSize = true };
        var synchronize = new Button { Text = UiText.T("Actualiser les passkeys disponibles dans Windows"), AutoSize = true };
        foreach (var button in new[] { identity, register, settings, hello, synchronize }) { button.Margin = new Padding(0, 0, 0, 10); actions.Controls.Add(button); }
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 3 };
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 100)); layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100)); layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 95));
        state.Dock = details.Dock = actions.Dock = DockStyle.Fill; layout.Controls.Add(state, 0, 0); layout.Controls.Add(actions, 0, 1); layout.Controls.Add(details, 0, 2); Controls.Add(layout);
        identity.Click += async (_, _) => await InstallIdentityAsync();
        register.Click += (_, _) => { try { ProviderActivation.Launch("--passkeys-register"); } catch (Exception error) { details.Text = UiText.T(error.Message); } };
        synchronize.Click += (_, _) => { try { ProviderActivation.Launch("--passkeys-sync"); } catch (Exception error) { details.Text = UiText.T(error.Message); } };
        settings.Click += (_, _) => Process.Start(new ProcessStartInfo("ms-settings:passkeys") { UseShellExecute = true });
        hello.Click += (_, _) => Process.Start(new ProcessStartInfo("ms-settings:signinoptions") { UseShellExecute = true });
        Activated += (_, _) => RefreshStatus(); RefreshStatus();
    }
    private void RefreshStatus()
    {
        var current = WindowsPasskeyApi.State();
        state.Text = UiText.T("API fournisseur : ") + (WindowsPasskeyApi.Available ? UiText.T("disponible") : UiText.T("indisponible")) + UiText.T("\nIdentité de ce processus : ") + (WindowsPasskeyApi.HasOwnIdentity ? "VaultAgent" : UiText.T("non activée")) +
            UiText.T("\nFournisseur : ") + (!current.Registered ? UiText.T("non enregistré") : current.Enabled ? UiText.T("activé") : UiText.T("à activer dans Windows")) + "\nWindows Hello : " + (HelloNative.Available ? UiText.T("disponible") : UiText.T("indisponible"));
        if (SystemInformation.TerminalServerSession) details.Text = UiText.T("Cette session est distante. Configurer et vérifier Windows Hello depuis la session locale du PC. Le coffre conserve son mode UAC.");
        else details.Text = UiText.T("Fournisseur expérimental : parcours complet à valider. Chaque création ou connexion demande Hello. Les clés restent dans le coffre chiffré.");
        identity.Enabled = WindowsPasskeyApi.Available && File.Exists(Path.Combine(AppPaths.Install, "passkeys", "passkey-package.json"));
        register.Enabled = WindowsPasskeyApi.Available && HelloNative.Available;
    }
    private async Task InstallIdentityAsync()
    {
        identity.Enabled = false;
        try {
            var directory = Path.Combine(AppPaths.Install, "passkeys");
            using var metadata = JsonDocument.Parse(await File.ReadAllTextAsync(Path.Combine(directory, "passkey-package.json")));
            var info = metadata.RootElement; var arguments = new List<string>();
            if (info.GetProperty("development").GetBoolean()) {
                var certificate = X509CertificateLoader.LoadCertificateFromFile(Path.Combine(directory, info.GetProperty("certificateFile").GetString()!));
                using (certificate) using (var store = new X509Store(StoreName.TrustedPeople, StoreLocation.CurrentUser)) {
                    store.Open(OpenFlags.ReadOnly);
                    if (store.Certificates.Find(X509FindType.FindByThumbprint, certificate.Thumbprint, false).Count == 0) {
                        var text = UiText.T("Ce package de développement utilise un certificat autosigné.\n\nÉditeur : ") + certificate.Subject + UiText.T("\nEmpreinte SHA256 : ") + certificate.GetCertHashString(System.Security.Cryptography.HashAlgorithmName.SHA256) +
                            UiText.T("\n\nL'ajouter à Personnes de confiance de votre compte pour autoriser les packages signés par ce certificat ? Aucun certificat d'autorité racine n'est ajouté.");
                        if (MessageBox.Show(this, text, UiText.T("Confiance du package VaultAgent"), MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
                        arguments.Add("-TrustDevelopmentCertificate");
                    }
                }
            }
            var process = new ProcessStartInfo(Path.Combine(Environment.SystemDirectory, @"WindowsPowerShell\v1.0\powershell.exe")) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardError = true, RedirectStandardOutput = true };
            foreach (var item in new[] { "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", Path.Combine(directory, "register-provider.ps1") }.Concat(arguments)) process.ArgumentList.Add(item);
            using var child = Process.Start(process) ?? throw new IOException(UiText.T("Enregistrement non démarré."));
            var error = child.StandardError.ReadToEndAsync(); var output = child.StandardOutput.ReadToEndAsync(); await child.WaitForExitAsync(); await output;
            if (child.ExitCode != 0) throw new InvalidOperationException(UiText.T("Identité Windows non enregistrée. ") + await error);
            if (!WindowsPasskeyApi.HasOwnIdentity) { ProviderActivation.Launch("--passkeys-settings"); Close(); return; }
            RefreshStatus(); details.Text = UiText.T("Identité installée. Enregistrer le fournisseur, puis l'activer dans les options avancées des passkeys Windows.");
        }
        catch (Exception error) { details.Text = UiText.T(error.Message); }
        finally { identity.Enabled = true; }
    }
}
