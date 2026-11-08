using System.Text.Json;
namespace Homelab.Vault;
internal static class AppPaths
{
    public const string RpId = "vaultagent-client.local";
    public const string Origin = "https://vaultagent-client.local";
    public static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);
}
internal sealed record HelloCredential(string Id, string KeyType, string First, string Second);
internal sealed record HelloProof(string CredentialId, string ClientData, string AuthenticatorData, string Signature);
