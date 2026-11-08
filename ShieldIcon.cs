using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace Homelab.Vault;

internal static class ShieldIcon
{
    [DllImport("user32.dll")] private static extern bool DestroyIcon(IntPtr icon);
    public static Bitmap Render(int size, bool unlocked = false)
    {
        var bitmap = new Bitmap(size, size);
        using var graphics = Graphics.FromImage(bitmap);
        graphics.SmoothingMode = SmoothingMode.AntiAlias;
        graphics.ScaleTransform(size / 256f, size / 256f);
        using var shield = new GraphicsPath();
        shield.AddLines(new PointF[] { new(128, 12), new(228, 52), new(228, 124) });
        shield.AddBezier(228, 124, 228, 181, 186, 222, 128, 246);
        shield.AddBezier(128, 246, 70, 222, 28, 181, 28, 124);
        shield.AddLines(new PointF[] { new(28, 124), new(28, 52), new(128, 12) }); shield.CloseFigure();
        using var purple = new LinearGradientBrush(new Point(28, 12), new Point(228, 246), Color.FromArgb(168, 121, 255), Color.FromArgb(104, 54, 199));
        graphics.FillPath(purple, shield);
        using var pen = new Pen(Color.White, 13) { StartCap = LineCap.Round, EndCap = LineCap.Round };
        graphics.DrawArc(pen, 96, 64, 64, 64, 180, 180); graphics.DrawLine(pen, 96, 96, 96, 121); graphics.DrawLine(pen, 160, 96, 160, 121);
        using var body = new GraphicsPath(); body.AddRoundedRectangle(new RectangleF(83, 109, 90, 70), new SizeF(17, 17));
        graphics.FillPath(Brushes.White, body);
        using var key = new SolidBrush(Color.FromArgb(121, 72, 217)); graphics.FillEllipse(key, 118, 128, 20, 20); graphics.FillRectangle(key, 123, 140, 10, 21);
        if (unlocked) { graphics.FillEllipse(Brushes.White, 177, 176, 62, 62); using var green = new SolidBrush(Color.FromArgb(31, 171, 115)); graphics.FillEllipse(green, 185, 184, 46, 46); }
        return bitmap;
    }
    public static Icon Create(int size = 32, bool unlocked = false)
    {
        using var bitmap = Render(size, unlocked); var handle = bitmap.GetHicon();
        try { return (Icon)Icon.FromHandle(handle).Clone(); } finally { DestroyIcon(handle); }
    }
    public static void WriteAssets(string directory)
    {
        Directory.CreateDirectory(directory);
        using (var preview = Render(256)) preview.Save(Path.Combine(directory, "shield.png"), System.Drawing.Imaging.ImageFormat.Png);
        var sizes = new[] { 16, 24, 32, 48, 64, 128, 256 };
        var images = sizes.Select(size => { using var bitmap = Render(size); using var buffer = new MemoryStream(); bitmap.Save(buffer, System.Drawing.Imaging.ImageFormat.Png); return buffer.ToArray(); }).ToArray();
        using var file = File.Create(Path.Combine(directory, "shield.ico")); using var writer = new BinaryWriter(file);
        writer.Write((ushort)0); writer.Write((ushort)1); writer.Write((ushort)sizes.Length);
        var offset = 6 + 16 * sizes.Length;
        for (var i = 0; i < sizes.Length; i++) { writer.Write((byte)(sizes[i] == 256 ? 0 : sizes[i])); writer.Write((byte)(sizes[i] == 256 ? 0 : sizes[i])); writer.Write((ushort)0); writer.Write((ushort)1); writer.Write((ushort)32); writer.Write(images[i].Length); writer.Write(offset); offset += images[i].Length; }
        foreach (var image in images) writer.Write(image);
    }
}
