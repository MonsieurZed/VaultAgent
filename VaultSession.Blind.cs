using System.Text.Json;

namespace Homelab.Vault;

internal sealed partial class VaultSession
{
    public void RequireAccess(Caller caller)
    {
        if (!IsUnlocked || SessionId != caller.SessionId) throw new UnauthorizedAccessException("Déverrouiller le coffre dans cette session Windows.");
    }
    public async Task<object> CatalogAsync(string search, Caller caller, CancellationToken token)
    {
        await serial.WaitAsync(token);
        try {
            RequireAccess(caller); lease.Touch();
            var result = await engine!.RequestAsync(new { op = "call", method = "GET", path = "/list/object/items?search=" + Uri.EscapeDataString(search) }, token);
            return result.GetProperty("data").EnumerateArray().Select(item => new {
                id = item.GetProperty("id").GetString(), name = item.GetProperty("name").GetString(),
                hasLogin = item.TryGetProperty("login", out var login) && login.ValueKind == JsonValueKind.Object,
                hasPasskey = item.TryGetProperty("login", out var passkeyLogin) && passkeyLogin.ValueKind == JsonValueKind.Object &&
                    passkeyLogin.TryGetProperty("fido2Credentials", out var keys) && keys.ValueKind == JsonValueKind.Array && keys.GetArrayLength() > 0,
                origins = Origins(item)
            }).ToArray();
        }
        finally { serial.Release(); }
    }
    private static string[] Origins(JsonElement item)
    {
        if (!item.TryGetProperty("login", out var login) || login.ValueKind != JsonValueKind.Object || !login.TryGetProperty("uris", out var uris) || uris.ValueKind != JsonValueKind.Array) return Array.Empty<string>();
        return uris.EnumerateArray().Select(value => value.TryGetProperty("uri", out var uri) && Uri.TryCreate(uri.GetString(), UriKind.Absolute, out var parsed) && parsed.Scheme is "https" or "http" ?
            new UriBuilder(parsed.Scheme, parsed.Host, parsed.Port).Uri.GetLeftPart(UriPartial.Authority) : null).Where(value => value != null).Cast<string>().Distinct().ToArray();
    }
    public async Task<string> SecretAsync(SecretRequest request, Caller caller, CancellationToken token)
    {
        if (!Guid.TryParse(request.ItemId, out _) || request.Field is not ("username" or "password" or "totp")) throw new ArgumentException("Fiche ou champ invalide.");
        await serial.WaitAsync(token);
        try {
            RequireAccess(caller);
            var field = request.Field;
            string value;
            if (field == "totp") {
                var item = await engine!.RequestAsync(new { op = "call", method = "GET", path = "/object/item/" + request.ItemId }, token);
                var period = 30;
                if (item.GetProperty("login").TryGetProperty("totp", out var key) && Uri.TryCreate(key.GetString(), UriKind.Absolute, out var uri) && uri.Scheme == "otpauth") {
                    foreach (var pair in uri.Query.TrimStart('?').Split('&')) {
                        var parts = pair.Split('=', 2);
                        if (parts.Length == 2 && parts[0] == "period" && int.TryParse(parts[1], out var configured) && configured is >= 5 and <= 300) period = configured;
                    }
                }
                var remaining = period * 1000L - DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() % (period * 1000L);
                if (remaining < 2000) await Task.Delay((int)remaining + 30, token);
                var result = await engine!.RequestAsync(new { op = "call", method = "GET", path = "/object/totp/" + request.ItemId }, token);
                value = result.GetProperty("data").GetString() ?? throw new InvalidOperationException("Code 2FA absent.");
            } else {
                var item = await engine!.RequestAsync(new { op = "call", method = "GET", path = "/object/item/" + request.ItemId }, token);
                value = item.GetProperty("login").GetProperty(field).GetString() ?? throw new InvalidOperationException("Champ absent de cette fiche.");
            }
            RequireAccess(caller); lease.Touch();
            return value;
        }
        finally { serial.Release(); }
    }
}
