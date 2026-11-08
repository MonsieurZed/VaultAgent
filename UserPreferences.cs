using System.Globalization;
using Microsoft.Win32;

namespace Homelab.Vault;

internal static class UserPreferences
{
    private const string Key = @"Software\MonsieurZed\VaultAgent\Preferences";
    public static string Language { get; private set; } = Read("Language", CultureInfo.CurrentUICulture.TwoLetterISOLanguageName is "fr" or "es" ? CultureInfo.CurrentUICulture.TwoLetterISOLanguageName : "en");
    public static string Channel { get; private set; } = Read("Channel", "stable");
    private static string Read(string name, string fallback) { using var key = Registry.CurrentUser.OpenSubKey(Key); return key?.GetValue(name) as string ?? fallback; }
    public static void Save(string language, string channel) {
        if (language is not ("fr" or "en" or "es") || channel is not ("stable" or "experimental")) throw new ArgumentException("Invalid preferences");
        using var key = Registry.CurrentUser.CreateSubKey(Key); key.SetValue("Language", language); key.SetValue("Channel", channel);
        Language = language; Channel = channel;
    }
}
