// Homelab VaultAgent · commande « type » : saisie dans une fenêtre dont l'identité est exigée.

using System.Text.Json;

namespace Homelab.Vault;

internal static class TypeCommand
{
    private const int MismatchExitCode = 3;

    /// <summary>
    /// Capture la fenêtre active, vérifie son identité avant de demander le secret, puis tape la valeur.
    /// </summary>
    /// <param name="args">Arguments après « type » : id-fiche, champ, --process, --title.</param>
    /// <returns>0 si la saisie est faite, 1 si le coffre reste verrouillé, 3 si la fenêtre est refusée.</returns>
    public static int Run(string[] args)
    {
        var (requirement, rest) = TargetRequirement.Parse(args);
        if (rest.Length != 2) throw new ArgumentException("Usage : type <id-fiche> <username|password|totp> --process <exe> --title <fragment>.");
        var target = WindowTarget.Foreground();
        try {
            DirectInput.Verify(target, requirement);
            var status = PipeTransport.SendAsync("status").GetAwaiter().GetResult();
            if (!status.GetProperty("unlocked").GetBoolean() && !UnlockFlow.RunDialog()) return 1;
            var result = PipeTransport.SendAsync("worker-secret", new SecretRequest(rest[0], rest[1])).GetAwaiter().GetResult();
            DirectInput.Type(result.GetProperty("value").GetString()!, target, requirement);
        } catch (TargetMismatchException refusal) { return Refuse(refusal); }
        Console.WriteLine(Serialize(new { typed = true, field = rest[1], clipboardUsed = false, target = target.Describe() }));
        return 0;
    }

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

    private static string Serialize(object value) => AppPaths.Element(value).GetRawText();
}
