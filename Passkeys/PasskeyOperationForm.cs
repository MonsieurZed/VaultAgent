using System.Text.Json;

namespace Homelab.Vault.Passkeys;

internal sealed class PasskeyOperationForm : Form
{
    private readonly SignedPasskeyRequest message;
    private readonly CancellationTokenSource stop = new();
    private readonly Label details = new() { Dock = DockStyle.Top, AutoSize = false, Height = 105 };
    private readonly Label status = new() { Dock = DockStyle.Bottom, Height = 70, ForeColor = Color.DimGray };
    private readonly ComboBox choices = new() { Dock = DockStyle.Top, DropDownStyle = ComboBoxStyle.DropDownList, Visible = false };
    private readonly Button confirm = new() { Text = UiText.T("Continuer avec Windows Hello"), Width = 255, Height = 38, Enabled = false };
    private JsonElement pending;
    private string? jobId;
    private bool disposed;
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public byte[]? Response { get; private set; }
    public PasskeyOperationForm(SignedPasskeyRequest message)
    {
        this.message = message; Text = "VaultAgent · " + (message.Create ? UiText.T("Enregistrer une passkey") : UiText.T("Connexion par passkey"));
        Icon = ShieldIcon.Create(64); Font = new Font("Segoe UI", 11); ClientSize = new Size(530, 330); Padding = new Padding(22); StartPosition = FormStartPosition.CenterScreen;
        FormBorderStyle = FormBorderStyle.FixedDialog; MaximizeBox = false;
        var cancel = new Button { Text = UiText.T("Annuler"), Height = 38, Width = 95 }; cancel.Click += (_, _) => Close(); CancelButton = cancel;
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 55, FlowDirection = FlowDirection.RightToLeft };
        confirm.BackColor = Color.FromArgb(121, 72, 217); confirm.ForeColor = Color.White; confirm.FlatStyle = FlatStyle.Flat;
        buttons.Controls.Add(cancel); buttons.Controls.Add(confirm); Controls.Add(choices); Controls.Add(details); Controls.Add(status); Controls.Add(buttons);
        details.Text = UiText.T("Vérification de la requête Windows…");
        Shown += async (_, _) => await PrepareAsync();
        confirm.Click += async (_, _) => await CompleteAsync();
        choices.SelectedIndexChanged += (_, _) => confirm.Enabled = choices.SelectedIndex >= 0;
        FormClosing += (_, _) => { stop.Cancel(); if (jobId != null && Response == null) _ = CancelAsync(jobId); };
    }
    private static async Task CancelAsync(string id) { try { await PipeTransport.SendAsync("passkey-cancel", new { jobId = id }); } catch { } }
    private async Task PrepareAsync()
    {
        try {
            await ProviderRegistration.RefreshKeysAsync(); stop.Token.ThrowIfCancellationRequested();
            await PipeTransport.SendAsync("passkey-inspect", message, token: stop.Token);
            await UnlockFlow.EnsureAsync(Handle, stop.Token);
            pending = await PipeTransport.SendAsync("passkey-begin", message, token: stop.Token); jobId = pending.GetProperty("jobId").GetString();
            details.Text = (message.Create ? UiText.T("Enregistrer une nouvelle passkey dans le coffre") : UiText.T("Se connecter avec une passkey du coffre")) + UiText.T("\nSite : ") + pending.GetProperty("rpId").GetString();
            if (message.Create) details.Text += UiText.T("\nCompte : ") + pending.GetProperty("userName").GetString();
            else {
                choices.Visible = true;
                foreach (var choice in pending.GetProperty("choices").EnumerateArray()) choices.Items.Add(new Choice(choice.GetProperty("credentialId").GetString()!, choice.GetProperty("userName").GetString()!, choice.GetProperty("displayName").GetString()!));
                choices.SelectedIndex = choices.Items.Count == 1 ? 0 : -1;
            }
            status.Text = UiText.T("Windows Hello confirme cette opération. La clé privée reste dans le service et le coffre chiffré.");
            confirm.Enabled = message.Create || choices.SelectedIndex >= 0;
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { details.Text = UiText.T("La passkey n'a pas été utilisée."); status.Text = UiText.T(error.Message); }
    }
    private async Task CompleteAsync()
    {
        confirm.Enabled = false;
        try {
            var selection = choices.SelectedItem as Choice; var user = message.Create ? pending.GetProperty("userName").GetString() ?? "" : selection?.User ?? "";
            var challenge = pending.GetProperty("challenge").GetBytesFromBase64(); var window = Handle; var rp = pending.GetProperty("rpId").GetString()!;
            var proof = await Task.Run(() => ProviderRegistration.VerifyUser(window, message.TransactionId, user, rp, challenge), stop.Token);
            stop.Token.ThrowIfCancellationRequested();
            var result = await PipeTransport.SendAsync("passkey-complete", new PasskeyCommit(jobId!, proof, selection?.Id), token: stop.Token);
            Response = result.GetProperty("response").GetBytesFromBase64();
            try { ProviderCache.Add(result); } catch (InvalidOperationException) { }
            Close();
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { status.Text = UiText.T(error.Message); }
    }
    private sealed record Choice(string Id, string User, string Display) { public override string ToString() => string.IsNullOrEmpty(Display) ? User : Display + " — " + User; }
    protected override void Dispose(bool disposing) { if (disposing && !disposed) { disposed = true; stop.Cancel(); stop.Dispose(); } base.Dispose(disposing); }
}
