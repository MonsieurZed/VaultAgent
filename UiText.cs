using System.Reflection;
using System.Text.Json;

namespace Homelab.Vault;

internal static class UiText
{
    private static readonly Dictionary<string, string[]> Catalog = Load();
    private static Dictionary<string, string[]> Load() {
        using var stream = typeof(UiText).Assembly.GetManifestResourceStream("VaultAgent.Translations.json")!;
        return JsonSerializer.Deserialize<Dictionary<string, string[]>>(stream)!;
    }
    internal static string? PreviewLanguage { get; set; }
    public static string T(string french) => Translate(french, PreviewLanguage ?? UserPreferences.Language);
    public static string Translate(string french, string language) => language == "fr" || !Catalog.TryGetValue(french, out var values) ? french : values[language == "es" ? 1 : 0];
    public static IReadOnlyDictionary<string, string[]> Entries => Catalog;
}
