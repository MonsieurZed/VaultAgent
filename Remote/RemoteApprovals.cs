using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Homelab.Vault.Remote;

internal static partial class RemoteConnection
{
    private static string ApprovalFile => Path.Combine(Path.GetDirectoryName(FileName)!, "approval-key.dpapi");
    private static RSA ApprovalKey()
    {
        Directory.CreateDirectory(Path.GetDirectoryName(ApprovalFile)!);
        if (!File.Exists(ApprovalFile)) {
            using var generated = RSA.Create(2048); var bytes = generated.ExportPkcs8PrivateKey();
            try { using var file = new FileStream(ApprovalFile, FileMode.CreateNew); file.Write(ProtectedData.Protect(bytes, Entropy, DataProtectionScope.CurrentUser)); }
            finally { CryptographicOperations.ZeroMemory(bytes); }
        }
        var clear = ProtectedData.Unprotect(File.ReadAllBytes(ApprovalFile), Entropy, DataProtectionScope.CurrentUser);
        try { var rsa = RSA.Create(); rsa.ImportPkcs8PrivateKey(clear, out _); return rsa; }
        finally { CryptographicOperations.ZeroMemory(clear); }
    }
    // Seuls l'identifiant public et la cle publique sortent lors de l'installation.
    public static object ApprovalIdentity()
    {
        using var rsa = ApprovalKey(); var settings = Read();
        var token = ProtectedData.Unprotect(Convert.FromBase64String(settings.ProtectedToken), Entropy, DataProtectionScope.CurrentUser);
        try { return new { deviceId = Encoding.UTF8.GetString(token).Split('.')[1], publicKey = Convert.ToBase64String(rsa.ExportSubjectPublicKeyInfo()) }; }
        finally { CryptographicOperations.ZeroMemory(token); }
    }
    internal static async Task DecideAsync(JsonElement request, bool approve, bool remember)
    {
        using var rsa = ApprovalKey(); var server = ValidateServer(Read().Server);
        var requestId = request.GetProperty("requestId").GetString(); var challenge = request.GetProperty("challenge").GetString();
        var options = new JsonSerializerOptions { Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping };
        var message = JsonSerializer.Serialize(new object?[] { "VaultAgent.Unlock.v1", server, requestId, challenge, approve, remember }, options);
        var signature = rsa.SignData(Encoding.UTF8.GetBytes(message), HashAlgorithmName.SHA256, RSASignaturePadding.Pss);
        await CallAsync("decide-unlock", new { requestId, approve, remember, signature = Convert.ToBase64String(signature) });
    }
    internal static async Task ResetAutomaticAsync() => await DecideAsync(await CallAsync("approval-context", new { }), false, false);
}

// Petite carte de notification ; aucun secret du coffre n'est charge dans cette fenetre.
internal sealed class RemoteApprovalForm : Form
{
    private readonly System.Windows.Forms.Timer expiry = new() { Interval = 1000 };
    private bool deciding;
    public string RequestId { get; }
    public RemoteApprovalForm(JsonElement request)
    {
        RequestId = request.GetProperty("requestId").GetString()!;
        Text = "VaultAgent"; Icon = ShieldIcon.Create(); TopMost = true; ShowInTaskbar = false;
        FormBorderStyle = FormBorderStyle.FixedToolWindow; StartPosition = FormStartPosition.Manual;
        ClientSize = new Size(420, 210); BackColor = Color.FromArgb(249, 246, 255);
        var area = Screen.PrimaryScreen!.WorkingArea; Location = new Point(area.Right - Width - 20, area.Bottom - Height - 20);
        var title = new Label { Text = UiText.T("Autoriser l'accès au coffre ?"), Left = 20, Top = 18, Width = 380, Height = 28, Font = new Font("Segoe UI", 13, FontStyle.Bold) };
        var name = new Label { Text = request.GetProperty("deviceName").GetString(), Left = 20, Top = 55, Width = 380, Height = 35, AutoEllipsis = true };
        var always = new CheckBox { Text = UiText.T("Toujours accepter les demandes de ce client"), Left = 20, Top = 100, Width = 380, Height = 28 };
        var yes = new Button { Text = UiText.T("Oui"), Left = 200, Top = 155, Width = 90, Height = 34, BackColor = Color.FromArgb(112, 65, 199), ForeColor = Color.White, FlatStyle = FlatStyle.Flat };
        var no = new Button { Text = UiText.T("Non"), Left = 300, Top = 155, Width = 90, Height = 34 };
        var end = request.GetProperty("expiresUtc").GetDateTimeOffset();
        async Task Decide(bool approved) {
            if (deciding) return; deciding = true; yes.Enabled = no.Enabled = always.Enabled = false;
            try { await RemoteConnection.DecideAsync(request, approved, approved && always.Checked); Close(); }
            catch { title.Text = UiText.T("Demande expirée ou opération refusée"); expiry.Stop(); await Task.Delay(2500); Close(); }
        }
        yes.Click += async (_, _) => await Decide(true); no.Click += async (_, _) => await Decide(false);
        Controls.AddRange(new Control[] { title, name, always, yes, no }); ActiveControl = no;
        expiry.Tick += (_, _) => { if (DateTimeOffset.UtcNow >= end && !deciding) Close(); }; expiry.Start();
    }
    protected override bool ShowWithoutActivation => true;
    protected override void Dispose(bool disposing) { if (disposing) { expiry.Dispose(); Icon?.Dispose(); } base.Dispose(disposing); }
}
