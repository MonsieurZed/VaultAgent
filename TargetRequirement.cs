// Homelab VaultAgent · identité exigée de la fenêtre cible d'une saisie directe.
// Logique pure, sans appel système, testée par --self-test.

namespace Homelab.Vault;

/// <summary>
/// Conditions que la fenêtre active doit remplir avant et pendant la frappe :
/// nom de processus exact et fragment de titre explicite.
/// </summary>
internal sealed record TargetRequirement(string Process, string Title)
{
    private const int MinimumTitleLength = 3;
    private static readonly string[] GenericTitles = {
        "new tab", "nouvel onglet", "google chrome", "chrome", "microsoft edge", "edge", "firefox", "mozilla firefox",
        "opera", "brave", "vivaldi", "about:blank", "untitled", "sans titre", "nouvelle fenetre", "new window"
    };

    /// <summary>Valide et normalise les exigences ; refuse les titres trop génériques pour identifier une page.</summary>
    /// <param name="process">Nom du processus, avec ou sans .exe.</param>
    /// <param name="title">Fragment de titre qui doit figurer dans la fenêtre.</param>
    /// <returns>Exigences normalisées.</returns>
    public static TargetRequirement Create(string? process, string? title)
    {
        var name = (process ?? "").Trim();
        if (name.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)) name = name[..^4];
        if (name.Length == 0 || name.Length > 64 || !name.All(c => char.IsAsciiLetterOrDigit(c) || c is '.' or '_' or '-'))
            throw new ArgumentException("--process doit être un nom de processus simple, par exemple chrome.");
        var fragment = (title ?? "").Trim();
        if (fragment.Length < MinimumTitleLength || fragment.Any(char.IsControl))
            throw new ArgumentException("--title doit contenir au moins " + MinimumTitleLength + " caractères visibles du titre de la page ou de l'application.");
        var folded = Fold(fragment);
        if (GenericTitles.Any(generic => generic.Contains(folded, StringComparison.Ordinal)))
            throw new ArgumentException("--title est trop générique (nom du navigateur ou onglet vide) ; fournir un fragment propre à la page cible.");
        return new TargetRequirement(name, fragment);
    }

    /// <summary>Extrait --process et --title des arguments et renvoie les arguments positionnels restants.</summary>
    /// <param name="args">Arguments de ligne de commande après le nom de la commande.</param>
    /// <returns>Exigences et arguments positionnels.</returns>
    public static (TargetRequirement Requirement, string[] Positional) Parse(string[] args)
    {
        string? process = null, title = null; var rest = new List<string>();
        for (var index = 0; index < args.Length; index++) {
            if (args[index] is "--process" or "--title") {
                if (index + 1 >= args.Length) throw new ArgumentException(args[index] + " attend une valeur.");
                if (args[index] == "--process") process = args[++index]; else title = args[++index];
            } else if (args[index].StartsWith("--", StringComparison.Ordinal)) throw new ArgumentException("Option inconnue : " + args[index]);
            else rest.Add(args[index]);
        }
        if (process is null || title is null) throw new ArgumentException("La saisie directe exige --process <exe> et --title <fragment> pour identifier la fenêtre cible.");
        return (Create(process, title), rest.ToArray());
    }

    /// <summary>Indique si une fenêtre observée satisfait les exigences.</summary>
    /// <param name="processName">Nom du processus propriétaire.</param>
    /// <param name="windowTitle">Titre courant de la fenêtre.</param>
    /// <returns>Vrai si le processus est identique et le titre contient le fragment.</returns>
    public bool Matches(string processName, string windowTitle) =>
        string.Equals(processName, Process, StringComparison.OrdinalIgnoreCase) &&
        windowTitle.Contains(Title, StringComparison.OrdinalIgnoreCase);

    private static string Fold(string value) =>
        new string(value.Normalize(System.Text.NormalizationForm.FormD).Where(c => System.Globalization.CharUnicodeInfo.GetUnicodeCategory(c) != System.Globalization.UnicodeCategory.NonSpacingMark).ToArray()).ToLowerInvariant();
}
