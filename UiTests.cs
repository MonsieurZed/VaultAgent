using System.Text.Json;
using System.Runtime.InteropServices;

namespace Homelab.Vault;

internal static class UiTests
{
    public static void RenderRemote(string path, string language)
    {
        UiText.PreviewLanguage = language;
        using var form = new Remote.RemoteClientForm(preview: true) { Opacity = 0, ShowInTaskbar = false };
        using var timer = new System.Windows.Forms.Timer { Interval = 300 };
        timer.Tick += (_, _) => { timer.Stop(); using var bitmap = new Bitmap(form.Width, form.Height); form.DrawToBitmap(bitmap, new Rectangle(Point.Empty, bitmap.Size)); bitmap.Save(path); form.Close(); };
        form.Shown += (_, _) => timer.Start(); Application.Run(form);
    }
    public static void RenderPreferences(string path, string language)
    {
        UiText.PreviewLanguage = language;
        using var form = new ApplicationSettingsForm { Opacity = 0, ShowInTaskbar = false };
        using var timer = new System.Windows.Forms.Timer { Interval = 300 };
        timer.Tick += (_, _) => { timer.Stop(); using var bitmap = new Bitmap(form.Width, form.Height); form.DrawToBitmap(bitmap, new Rectangle(Point.Empty, bitmap.Size)); bitmap.Save(path); form.Close(); };
        form.Shown += (_, _) => timer.Start(); Application.Run(form);
    }
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr window);
    public static void RenderPasskeys(string path)
    {
        using var form = new Passkeys.PasskeySettingsForm();
        form.ShowInTaskbar = false; form.Opacity = 0;
        using var timer = new System.Windows.Forms.Timer { Interval = 300 };
        timer.Tick += (_, _) => {
            timer.Stop(); using var bitmap = new Bitmap(form.Width, form.Height);
            form.DrawToBitmap(bitmap, new Rectangle(Point.Empty, bitmap.Size)); bitmap.Save(path); form.Close();
        };
        form.Shown += (_, _) => timer.Start(); Application.Run(form);
    }
    public static void Render(string path, int tab = 0)
    {
        using var form = new AccountForm(preview: true);
        form.Opacity = 0; form.ShowInTaskbar = false;
        using var timer = new System.Windows.Forms.Timer { Interval = 400 };
        timer.Tick += (_, _) => {
            timer.Stop(); using var bitmap = new Bitmap(form.Width, form.Height);
            form.DrawToBitmap(bitmap, new Rectangle(Point.Empty, bitmap.Size)); bitmap.Save(path); form.Close();
        };
        form.Shown += (_, _) => { form.Controls.OfType<TabControl>().Single().SelectedIndex = tab; timer.Start(); }; Application.Run(form);
    }
    public static void Input(string receipt)
    {
        const string sample = "Test-éè-Unicode-42!";
        using var form = new Form { Text = "Test de saisie directe · valeur factice", ClientSize = new Size(420, 130), StartPosition = FormStartPosition.CenterScreen, Icon = ShieldIcon.Create(), TopMost = true };
        var box = new TextBox { UseSystemPasswordChar = true, Bounds = new Rectangle(25, 45, 360, 28) }; form.Controls.Add(box);
        var step = 0; Exception? failure = null;
        using var timer = new System.Windows.Forms.Timer { Interval = 300 };
        timer.Tick += (_, _) => {
            if (step++ == 0) {
                SetForegroundWindow(form.Handle); form.Activate(); box.Focus();
            } else if (step == 2) {
                try { DirectInput.Type(sample, form.Handle); } catch (Exception error) { failure = error; }
            } else {
                timer.Stop(); File.WriteAllText(receipt, JsonSerializer.Serialize(new { success = failure == null && box.Text == sample, clipboardUsed = false, unicode = true, error = failure?.Message, targetFocused = DirectInput.GetForegroundWindow() == form.Handle, characterCount = box.Text.Length }, AppPaths.Json)); form.Close();
            }
        };
        form.Shown += (_, _) => timer.Start(); Application.Run(form);
    }
}
