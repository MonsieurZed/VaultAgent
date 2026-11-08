namespace Homelab.Vault.Passkeys;

internal static class ProviderSynchronization
{
    public static void Run()
    {
        if (!WindowsPasskeyApi.HasOwnIdentity) throw new InvalidOperationException("Activer l'identité du fournisseur.");
        if (!UnlockFlow.RunDialog()) return;
        ProviderRegistration.RefreshKeysAsync().GetAwaiter().GetResult();
        var metadata = PipeTransport.SendAsync("passkey-metadata").GetAwaiter().GetResult();
        ProviderCache.Clear(); var count = 0;
        foreach (var entry in metadata.EnumerateArray()) { ProviderCache.Add(entry); count++; }
        MessageBox.Show(count + " passkey(s) disponibles dans Windows.", "VaultAgent");
    }
}
