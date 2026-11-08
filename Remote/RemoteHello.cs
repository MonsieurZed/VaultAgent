using System.Security.Cryptography;
using System.Security.Principal;
using System.Text;
using System.Text.Json;

namespace Homelab.Vault.Remote;
internal static partial class RemoteConnection
{
    internal const string HelloRp = "vaultagent-client.local";
    internal const string HelloOrigin = "https://vaultagent-client.local";
    internal static async Task<int> EnrollHelloAsync(IntPtr window)
    {
        if (!HelloNative.Available) throw new InvalidOperationException("Windows Hello indisponible.");
        var available = await CallAsync("vaults", new { }); var targets = new List<string>();
        foreach (var vault in available.EnumerateArray()) {
            var id = vault.GetProperty("id").GetString()!;
            if (TargetVault is { } selected && id != selected) continue;
            var state = await CallAsync("status", new { }, id); var security = state.GetProperty("approvalSecurity");
            if (security.GetProperty("registered").GetBoolean() && !(security.GetProperty("helloEnabled").GetBoolean() && security.GetProperty("helloRequiredForAll").GetBoolean())) targets.Add(id);
        }
        if (targets.Count == 0) return 0;
        // Le PIN et la biometrie restent dans l'interface Windows.
        HelloCredential? created = null;
        var count = 0;
        foreach (var id in targets) {
            var challenge = await CallAsync("hello-begin", new { }, id);
            HelloCredential credential;
            if (challenge.GetProperty("helloCredential").ValueKind != JsonValueKind.Null) credential = challenge.GetProperty("helloCredential").Deserialize<HelloCredential>(AppPaths.Json)!;
            else {
                created ??= HelloNative.Register(window, WindowsIdentity.GetCurrent().User!.Value, "VaultAgent", HelloRp, HelloOrigin);
                credential = created; challenge = await CallAsync("hello-begin", new { }, id);
            }
            var message = DecisionMessage(challenge, true, false);
            var digest = HelloVerification.Base64Url(SHA256.HashData(Encoding.UTF8.GetBytes(message)));
            var proof = HelloNative.Sign(window, credential, digest, HelloRp, HelloOrigin); HelloVerification.Verify(credential, proof, digest, HelloRp, HelloOrigin);
            await CallAsync("hello-enroll", new { credential, helloProof = proof, signature = ApprovalSignature(message) }, id); ++count;
        }
        return count;
    }
}
internal sealed class RemoteHelloForm : Form
{
    public RemoteHelloForm()
    {
        Text = "VaultAgent · Windows Hello"; Icon = ShieldIcon.Create(); ClientSize = new Size(480, 230);
        StartPosition = FormStartPosition.CenterScreen; Font = new Font("Segoe UI", 10); FormBorderStyle = FormBorderStyle.FixedDialog; MaximizeBox = false;
        var description = new Label { Left = 22, Top = 18, Width = 435, Height = 88, Text = UiText.T("L'activation verrouille les coffres concernés et impose Hello aux PC approbateurs. Linux vérifie chaque preuve. Les acceptations automatiques restent actives.") };
        var status = new Label { Left = 22, Top = 112, Width = 435, Height = 58 };
        var activate = new Button { Left = 22, Top = 182, Width = 260, Height = 32, Text = UiText.T("Activer Windows Hello") };
        var settings = new Button { Left = 294, Top = 182, Width = 164, Height = 32, Text = UiText.T("Options Windows") };
        settings.Click += (_, _) => System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo("ms-settings:signinoptions") { UseShellExecute = true });
        if (!HelloNative.Available) { activate.Enabled = false; status.Text = UiText.T("Windows Hello indisponible. Configurez-le dans Windows depuis une session locale ; le RDP peut empêcher son utilisation."); }
        activate.Click += async (_, _) => {
            activate.Enabled = false;
            try { var count = await RemoteConnection.EnrollHelloAsync(Handle); status.Text = UiText.T(count > 0 ? "Windows Hello activé sur les coffres autorisés." : "Aucun coffre à activer : déjà configuré ou PC non approbateur."); }
            catch { status.Text = UiText.T("Windows Hello annulé ou opération refusée. Aucune validation de secours automatique."); }
            finally { activate.Enabled = HelloNative.Available; }
        };
        Controls.AddRange(new Control[] { description, status, activate, settings });
    }
}
