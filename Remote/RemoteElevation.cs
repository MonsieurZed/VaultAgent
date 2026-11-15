using System.ComponentModel;
using System.Diagnostics;
using System.Security.AccessControl;
using System.Security.Cryptography;
using System.Security.Principal;
using System.Text;
using System.Text.Json;

namespace Homelab.Vault.Remote;

// Preuve d'élévation : la clé privée n'est lisible que par un processus administrateur.
// Seule une validation UAC permet de signer ; le serveur vérifie la signature avec la clé publique enrôlée.
internal static partial class RemoteConnection
{
    private static string ElevationPath(string sid) =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "VaultAgent", "elevation", sid + ".key");

    private static void RequireAdministrator()
    {
        using var identity = WindowsIdentity.GetCurrent();
        if (!new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator)) throw new UnauthorizedAccessException("Élévation requise.");
    }

    private static void EnsureElevationDirectory(string directory)
    {
        if (Directory.Exists(directory)) return;
        var security = new DirectorySecurity(); security.SetAccessRuleProtection(true, false);
        foreach (var account in new[] { WellKnownSidType.LocalSystemSid, WellKnownSidType.BuiltinAdministratorsSid })
            security.AddAccessRule(new FileSystemAccessRule(new SecurityIdentifier(account, null), FileSystemRights.FullControl, InheritanceFlags.ContainerInherit | InheritanceFlags.ObjectInherit, PropagationFlags.None, AccessControlType.Allow));
        new DirectoryInfo(directory).Create(security);
    }

    // Exécutée par le processus élevé : crée la clé au besoin, puis signe le message.
    internal static (string PublicKey, string Signature) ElevationSign(string sid, string message)
    {
        RequireAdministrator();
        var path = ElevationPath(sid); EnsureElevationDirectory(Path.GetDirectoryName(path)!);
        using var rsa = RSA.Create();
        if (File.Exists(path)) {
            var clear = ProtectedData.Unprotect(File.ReadAllBytes(path), Entropy, DataProtectionScope.LocalMachine);
            try { rsa.ImportPkcs8PrivateKey(clear, out _); } finally { CryptographicOperations.ZeroMemory(clear); }
        } else {
            rsa.KeySize = 2048; var bytes = rsa.ExportPkcs8PrivateKey();
            try { using var file = new FileStream(path, FileMode.CreateNew); file.Write(ProtectedData.Protect(bytes, Entropy, DataProtectionScope.LocalMachine)); }
            finally { CryptographicOperations.ZeroMemory(bytes); }
        }
        var signature = rsa.SignData(Encoding.UTF8.GetBytes(message), HashAlgorithmName.SHA256, RSASignaturePadding.Pss);
        return (Convert.ToBase64String(rsa.ExportSubjectPublicKeyInfo()), Convert.ToBase64String(signature));
    }

    // Lance le processus élevé : l'invite UAC est la preuve de présence ; l'annulation lève une exception.
    private static (string PublicKey, string Signature) ElevatedSign(string message)
    {
        var output = Path.Combine(Path.GetTempPath(), "vaultagent-" + Guid.NewGuid().ToString("N") + ".txt");
        var sid = WindowsIdentity.GetCurrent().User!.Value;
        try {
            var info = new ProcessStartInfo(Environment.ProcessPath!, $"elevation-sign {sid} {Convert.ToBase64String(Encoding.UTF8.GetBytes(message))} \"{output}\"") { UseShellExecute = true, Verb = "runas", WindowStyle = ProcessWindowStyle.Hidden };
            using var process = Process.Start(info) ?? throw new InvalidOperationException("Validation Windows indisponible.");
            if (!process.WaitForExit(120000)) { process.Kill(); throw new InvalidOperationException("Validation Windows expirée."); }
            if (process.ExitCode != 0 || !File.Exists(output)) throw new InvalidOperationException("Validation Windows refusée.");
            var lines = File.ReadAllLines(output); if (lines.Length != 2) throw new InvalidOperationException("Réponse élevée invalide.");
            return (lines[0], lines[1]);
        }
        catch (Win32Exception) { throw new InvalidOperationException("Validation Windows annulée."); }
        finally { try { File.Delete(output); } catch (IOException) { } }
    }

    internal static string ElevationSignature(string message) => ElevatedSign(message).Signature;

    internal static async Task<int> EnrollElevationAsync()
    {
        var count = 0; var vaults = await CallAsync("vaults", new { });
        foreach (var vault in vaults.EnumerateArray()) {
            var id = vault.GetProperty("id").GetString()!;
            var security = (await CallAsync("status", new { }, id)).GetProperty("approvalSecurity");
            if (!security.GetProperty("registered").GetBoolean() || security.TryGetProperty("elevationEnabled", out var enabled) && enabled.GetBoolean()) continue;
            var message = DecisionMessage(await CallAsync("elevation-begin", new { }, id), true, false);
            var (publicKey, elevationSignature) = ElevatedSign(message);
            await CallAsync("elevation-enroll", new { publicKey, signature = ApprovalSignature(message), elevationSignature }, id); ++count;
        }
        return count;
    }
}
