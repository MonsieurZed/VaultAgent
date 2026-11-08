using System.Diagnostics;
using System.Security.Principal;
using System.Text.Json;

namespace Homelab.Vault;

internal static class AppPaths
{
    public const string ServiceName = "HomelabVaultAgent";
    public const string PipeName = "HomelabVault.Agent.v2";
    public const string RpId = "homelab-vault.local";
    public const string Origin = "https://homelab-vault.local";
    public static string Install => AppContext.BaseDirectory;
    public static string Data => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "HomelabVaultAgent");
    public static string Sid => WindowsIdentity.GetCurrent().User!.Value;
    public static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);
    public static JsonElement Element(object value) => JsonSerializer.SerializeToElement(value, Json);
}

internal sealed record Request(string Op, JsonElement Payload, string? OwnerSid = null);
internal sealed record Reply(bool Success, JsonElement Data, string? Error = null)
{
    public static Reply Ok(object value) => new(true, AppPaths.Element(value));
    public static Reply Fail(string message) => new(false, AppPaths.Element(new { }), message);
}
internal sealed record HelloCredential(string Id, string KeyType, string First, string Second);
internal sealed record HelloProof(string CredentialId, string ClientData, string AuthenticatorData, string Signature);
internal sealed record AccountSettings(string Sid, string Email, string Server, string Mode, int? TimeoutMinutes,
    string ProtectedPassword, string ProfileId, HelloCredential? Hello, string? CertificatePem, bool BlindMode = true)
{
    public object Public() => new { configured = true, email = Email, server = Server, mode = Mode,
        timeoutMinutes = TimeoutMinutes, hello = Hello, certificateConfigured = CertificatePem != null,
        blindMode = BlindMode };
}
internal sealed record ConfigureAccount(string Email, string Server, string Mode, int? TimeoutMinutes,
    string? Password, string? TwoFactorCode, HelloCredential? Hello, string? CertificatePem, bool BlindMode = true);

internal sealed class SlidingLease(Func<long>? clock = null)
{
    private long Now => clock?.Invoke() ?? Stopwatch.GetTimestamp();
    private long deadline;
    public bool Authorized { get; private set; }
    public int? Minutes { get; private set; }
    public DateTimeOffset? ExpiresUtc { get; private set; }
    public bool IsValid => Authorized && (Minutes == null || Now < Interlocked.Read(ref deadline));
    public void Open(int? minutes) { Minutes = minutes; Authorized = true; Touch(); }
    public void Touch()
    {
        if (!Authorized) return;
        ExpiresUtc = Minutes is int minutes ? DateTimeOffset.UtcNow.AddMinutes(minutes) : null;
        Interlocked.Exchange(ref deadline, Minutes is int value ? Now + value * 60L * Stopwatch.Frequency : long.MaxValue);
    }
    public void Close() { Authorized = false; ExpiresUtc = null; Interlocked.Exchange(ref deadline, 0); }
}
