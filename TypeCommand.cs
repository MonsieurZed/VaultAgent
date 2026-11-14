// Homelab VaultAgent · commande « type » : saisie dans une fenêtre dont l'identité est exigée.

namespace Homelab.Vault;

internal static class TypeCommand
{
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
        } catch (TargetMismatchException refusal) { return TargetReport.Refuse(refusal); }
        Console.WriteLine(TargetReport.Serialize(new { typed = true, field = rest[1], clipboardUsed = false, target = target.Describe() }));
        return 0;
    }
}
