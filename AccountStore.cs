using System.Security.Cryptography;
using System.Security.Principal;
using System.Text;
using System.Text.Json;

namespace Homelab.Vault;

internal sealed class AccountStore(string root)
{
    public string DirectoryFor(string sid)
    {
        _ = new SecurityIdentifier(sid);
        return Path.Combine(root, sid);
    }
    public AccountSettings? Load(string sid)
    {
        var file = Path.Combine(DirectoryFor(sid), "account.json");
        if (!File.Exists(file)) return null;
        var result = JsonSerializer.Deserialize<AccountSettings>(File.ReadAllText(file), AppPaths.Json);
        if (result?.Sid != sid) throw new InvalidOperationException("Compte local invalide.");
        return result;
    }
    public void Save(AccountSettings value)
    {
        var directory = DirectoryFor(value.Sid);
        Directory.CreateDirectory(directory);
        var file = Path.Combine(directory, "account.json");
        var temporary = Path.Combine(directory, "account-" + Guid.NewGuid().ToString("N") + ".tmp");
        File.WriteAllText(temporary, JsonSerializer.Serialize(value, AppPaths.Json));
        File.Move(temporary, file, true);
    }
    public void Forget(string sid) => File.Delete(Path.Combine(DirectoryFor(sid), "account.json"));
    public static string Protect(string sid, string password)
    {
        var input = Encoding.UTF8.GetBytes(password);
        try { return Convert.ToBase64String(ProtectedData.Protect(input, Encoding.UTF8.GetBytes(sid), DataProtectionScope.LocalMachine)); }
        finally { CryptographicOperations.ZeroMemory(input); }
    }
    public static string Unprotect(AccountSettings value)
    {
        var bytes = ProtectedData.Unprotect(Convert.FromBase64String(value.ProtectedPassword), Encoding.UTF8.GetBytes(value.Sid), DataProtectionScope.LocalMachine);
        try { return Encoding.UTF8.GetString(bytes); }
        finally { CryptographicOperations.ZeroMemory(bytes); }
    }
    public string ProfileFor(AccountSettings value) => Path.Combine(DirectoryFor(value.Sid), "profiles", value.ProfileId);
}
