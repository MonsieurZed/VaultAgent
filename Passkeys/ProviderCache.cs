using System.Runtime.InteropServices;
using System.Text.Json;

namespace Homelab.Vault.Passkeys;

internal static class ProviderCache
{
    [StructLayout(LayoutKind.Sequential)] private struct CredentialDetails {
        public uint IdLength; public IntPtr Id, RpId, RpName; public uint UserLength; public IntPtr User, UserName, DisplayName;
    }
    [DllImport("webauthn.dll")] private static extern int WebAuthNPluginAuthenticatorAddCredentials(in Guid clsid, uint count, in CredentialDetails credential);
    [DllImport("webauthn.dll")] private static extern int WebAuthNPluginAuthenticatorRemoveAllCredentials(in Guid clsid);
    public static void Clear() => WindowsPasskeyApi.Check(WebAuthNPluginAuthenticatorRemoveAllCredentials(WindowsPasskeyApi.ClassId));
    public static void Add(JsonElement metadata) {
        using var memory = new NativeMemory();
        var id = metadata.GetProperty("credentialId").GetBytesFromBase64(); var user = metadata.GetProperty("userId").GetBytesFromBase64();
        var value = new CredentialDetails { IdLength = (uint)id.Length, Id = memory.Bytes(id), RpId = memory.Text(metadata.GetProperty("rpId").GetString()!),
            RpName = memory.Text(metadata.GetProperty("rpName").GetString()!), UserLength = (uint)user.Length, User = memory.Bytes(user),
            UserName = memory.Text(metadata.GetProperty("userName").GetString()!), DisplayName = memory.Text(metadata.GetProperty("displayName").GetString()!) };
        WindowsPasskeyApi.Check(WebAuthNPluginAuthenticatorAddCredentials(WindowsPasskeyApi.ClassId, 1, value));
    }
}
