using System.Net.Http;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Homelab.Vault.Remote;

internal sealed record RemoteSettings(string Server, string ProtectedToken);
internal static class RemoteConnection
{
    public const string DefaultServer = "https://vaultagent.lan.zed.cx";
    private static string FileName => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "VaultAgent", "remote-client.json");
    private static readonly byte[] Entropy = Encoding.UTF8.GetBytes("VaultAgent.Remote.v1");
    public static string ConfiguredServer { get { try { return Read().Server; } catch { return ""; } } }
    private static RemoteSettings Read() => JsonSerializer.Deserialize<RemoteSettings>(File.ReadAllText(FileName), AppPaths.Json) ?? throw new InvalidOperationException("Associer ce client au serveur VaultAgent.");
    private static string ValidateServer(string address) {
        if (!Uri.TryCreate(address, UriKind.Absolute, out var uri) || uri.Scheme != "https" || uri.UserInfo != "" || uri.Query != "" || uri.Fragment != "" || uri.AbsolutePath != "/") throw new ArgumentException("Adresse HTTPS du serveur VaultAgent requise.");
        return uri.GetLeftPart(UriPartial.Authority);
    }
    private static async Task<JsonElement> SendAsync(string server, string path, object payload, string? token = null) {
        using var client = new HttpClient(new HttpClientHandler { AllowAutoRedirect = false }) { Timeout = TimeSpan.FromSeconds(110) };
        using var request = new HttpRequestMessage(HttpMethod.Post, ValidateServer(server) + path);
        if (token != null) request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
        request.Content = new StringContent(JsonSerializer.Serialize(payload, AppPaths.Json), Encoding.UTF8, "application/json");
        using var response = await client.SendAsync(request, HttpCompletionOption.ResponseHeadersRead);
        if (!response.IsSuccessStatusCode) throw new InvalidOperationException(response.StatusCode == (System.Net.HttpStatusCode)423 ? "Coffre verrouille : autoriser les clients dans l interface du serveur." : "Serveur VaultAgent : requete refusee ou client revoque.");
        await using var stream = await response.Content.ReadAsStreamAsync(); using var buffer = new MemoryStream();
        var block = new byte[8192]; int count;
        while ((count = await stream.ReadAsync(block)) != 0) { if (buffer.Length + count > 4 * 1024 * 1024) throw new IOException("Reponse trop volumineuse."); buffer.Write(block, 0, count); }
        using var json = JsonDocument.Parse(buffer.ToArray()); return json.RootElement.Clone();
    }
    public static async Task PairAsync(string address, string code) {
        var server = ValidateServer(address.Trim()); var result = await SendAsync(server, "/api/pair", new { code });
        var token = Encoding.UTF8.GetBytes(result.GetProperty("token").GetString()!);
        try {
            var encrypted = ProtectedData.Protect(token, Entropy, DataProtectionScope.CurrentUser);
            Directory.CreateDirectory(Path.GetDirectoryName(FileName)!);
            var temporary = FileName + "." + Guid.NewGuid().ToString("N") + ".tmp";
            await File.WriteAllTextAsync(temporary, JsonSerializer.Serialize(new RemoteSettings(server, Convert.ToBase64String(encrypted)), AppPaths.Json)); File.Move(temporary, FileName, true);
        } finally { CryptographicOperations.ZeroMemory(token); }
    }
    public static async Task<JsonElement> CallAsync(string operation, object payload) {
        if (operation is not ("status" or "catalog" or "seal" or "lock" or "write" or "write-preview")) throw new ArgumentException("Operation distante refusee.");
        var settings = Read(); var token = ProtectedData.Unprotect(Convert.FromBase64String(settings.ProtectedToken), Entropy, DataProtectionScope.CurrentUser);
        try { return await SendAsync(settings.Server, "/api/agent/" + operation, payload, Encoding.UTF8.GetString(token)); }
        finally { CryptographicOperations.ZeroMemory(token); }
    }
    public static async Task<object> TypeAsync(string itemId, string field, string origin) {
        var target = DirectInput.GetForegroundWindow(); using var rsa = RSA.Create(2048);
        var result = await CallAsync("seal", new { itemId, field, origin, publicKey = Convert.ToBase64String(rsa.ExportSubjectPublicKeyInfo()) });
        var key = rsa.Decrypt(result.GetProperty("encryptedKey").GetBytesFromBase64(), RSAEncryptionPadding.OaepSHA256);
        var clear = new byte[result.GetProperty("data").GetBytesFromBase64().Length];
        try {
            using var aes = new AesGcm(key, 16); aes.Decrypt(result.GetProperty("nonce").GetBytesFromBase64(), result.GetProperty("data").GetBytesFromBase64(), result.GetProperty("tag").GetBytesFromBase64(), clear);
            DirectInput.Type(Encoding.UTF8.GetString(clear), target); return new { typed = true, field, clipboardUsed = false };
        } finally { CryptographicOperations.ZeroMemory(key); CryptographicOperations.ZeroMemory(clear); }
    }
}
