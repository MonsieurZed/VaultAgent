// Homelab VaultAgent · sorties JSON partagées de la saisie directe (service et client distant).

using System.Text.Json;

namespace Homelab.Vault;

internal static class TargetReport
{
    private const int MismatchExitCode = 3;

    /// <summary>Affiche le refus en JSON (raison, cible attendue, fenêtre active) sans aucun secret.</summary>
    /// <param name="refusal">Refus levé par la vérification d'identité.</param>
    /// <returns>Code de sortie dédié au refus de cible.</returns>
    public static int Refuse(TargetMismatchException refusal)
    {
        Console.WriteLine(Serialize(new { typed = false, reason = refusal.Message, expected = new { process = refusal.Requirement.Process, title = refusal.Requirement.Title }, foreground = refusal.Foreground?.Describe() }));
        return MismatchExitCode;
    }

    /// <summary>Affiche l'identité de la fenêtre active pour choisir --process et --title.</summary>
    /// <returns>Toujours 0.</returns>
    public static int Foreground()
    {
        Console.WriteLine(Serialize(WindowTarget.Foreground().Describe()));
        return 0;
    }

    /// <summary>Sérialise une valeur avec les options JSON communes.</summary>
    /// <param name="value">Valeur à sérialiser.</param>
    /// <returns>JSON compact.</returns>
    public static string Serialize(object value) => JsonSerializer.Serialize(value, AppPaths.Json);
}
