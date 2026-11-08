using System.ComponentModel;
using System.Diagnostics;
using System.Security.Principal;
using System.Text.Json;

namespace Homelab.Vault;

internal static class UnlockFlow
{
    public static bool Elevated => new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator);
    public static async Task ElevateAsync(params string[] args)
    {
        var info = new ProcessStartInfo(Path.Combine(AppPaths.Install, "VaultAgent.exe")) { UseShellExecute = true, Verb = "runas" };
        foreach (var arg in args) info.ArgumentList.Add(arg);
        try { using var process = Process.Start(info) ?? throw new IOException("Ouverture impossible."); await process.WaitForExitAsync(); if (process.ExitCode != 0) throw new InvalidOperationException("Opération Windows interrompue."); }
        catch (Win32Exception error) when (error.NativeErrorCode == 1223) { throw new InvalidOperationException("Validation UAC annulée."); }
    }
    public static async Task EnsureAsync(IntPtr window, CancellationToken token = default)
    {
        var status = await PipeTransport.SendAsync("status", token: token);
        if (status.GetProperty("unlocked").GetBoolean()) return;
        if (!status.GetProperty("configured").GetBoolean()) throw new InvalidOperationException("Configurer le compte depuis l'icône du bouclier violet.");
        var mode = status.GetProperty("mode").GetString();
        if (mode == "uac" || mode == "hello-or-uac" && !HelloNative.Available) {
            await ElevateAsync("--unlock-uac", "--owner", AppPaths.Sid); return;
        }
        if (!HelloNative.Available) throw new InvalidOperationException("Windows Hello n'est pas disponible pour ce compte Windows.");
        var request = await PipeTransport.SendAsync("challenge", token: token);
        var credential = request.GetProperty("credential").Deserialize<HelloCredential>(AppPaths.Json)!;
        var challenge = request.GetProperty("challenge").GetString()!;
        var proof = await Task.Run(() => HelloNative.Sign(window, credential, challenge), token);
        await PipeTransport.SendAsync("unlock-hello", proof, token: token);
    }
    public static bool RunDialog()
    {
        using var form = new Form { Text = "Déverrouiller le coffre", ClientSize = new Size(380, 110), StartPosition = FormStartPosition.CenterScreen, Icon = ShieldIcon.Create(), FormBorderStyle = FormBorderStyle.FixedDialog };
        form.Controls.Add(new Label { Text = "Validation Windows en cours…", AutoSize = true, Location = new Point(25, 35), Font = new Font("Segoe UI", 11) });
        var success = false;
        form.Shown += async (_, _) => { try { await EnsureAsync(form.Handle); success = true; } catch (Exception error) { MessageBox.Show(form, error.Message, "Coffre Bitwarden", MessageBoxButtons.OK, MessageBoxIcon.Error); } finally { form.Close(); } };
        form.ShowDialog(); return success;
    }
}
