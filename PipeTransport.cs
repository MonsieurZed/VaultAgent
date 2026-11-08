using System.Buffers.Binary;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Runtime.CompilerServices;
using System.Security.AccessControl;
using System.Security.Principal;
using System.Text.Json;
using Microsoft.Win32.SafeHandles;

namespace Homelab.Vault;

internal sealed record Caller(string Sid, bool Elevated, uint SessionId, uint ProcessId);
internal static class PipeTransport
{
    [DllImport("kernel32.dll", SetLastError = true)] private static extern bool GetNamedPipeServerProcessId(SafePipeHandle handle, out uint pid);
    [DllImport("kernel32.dll", SetLastError = true)] private static extern bool GetNamedPipeClientSessionId(SafePipeHandle handle, out uint session);
    [DllImport("kernel32.dll", SetLastError = true)] private static extern bool GetNamedPipeClientProcessId(SafePipeHandle handle, out uint pid);
    [DllImport("advapi32.dll", SetLastError = true)] private static extern bool GetTokenInformation(IntPtr token, int kind, out int result, int length, out int required);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)] private static extern IntPtr OpenSCManager(string? machine, string? database, uint access);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)] private static extern IntPtr OpenService(IntPtr manager, string name, uint access);
    [DllImport("advapi32.dll", SetLastError = true)] private static extern bool QueryServiceStatusEx(IntPtr service, int level, out ServiceState state, int size, out int needed);
    [DllImport("advapi32.dll")] private static extern bool CloseServiceHandle(IntPtr handle);
    [StructLayout(LayoutKind.Sequential)] private struct ServiceState { public uint Type, State, Accepted, Error, SpecificError, Checkpoint, Wait, Pid, Flags; }

    public static NamedPipeServerStream CreateServer(string name = AppPaths.PipeName)
    {
        var security = new PipeSecurity();
        security.SetAccessRuleProtection(true, false);
        security.SetOwner(new SecurityIdentifier(WellKnownSidType.LocalSystemSid, null));
        security.AddAccessRule(new PipeAccessRule(new SecurityIdentifier(WellKnownSidType.NetworkSid, null), PipeAccessRights.FullControl, AccessControlType.Deny));
        security.AddAccessRule(new PipeAccessRule(new SecurityIdentifier(WellKnownSidType.LocalSystemSid, null), PipeAccessRights.FullControl, AccessControlType.Allow));
        security.AddAccessRule(new PipeAccessRule(new SecurityIdentifier(WellKnownSidType.BuiltinAdministratorsSid, null), PipeAccessRights.FullControl, AccessControlType.Allow));
        security.AddAccessRule(new PipeAccessRule(new SecurityIdentifier(WellKnownSidType.AuthenticatedUserSid, null), PipeAccessRights.ReadWrite | PipeAccessRights.Synchronize, AccessControlType.Allow));
        return NamedPipeServerStreamAcl.Create(name, PipeDirection.InOut, 16, PipeTransmissionMode.Byte, PipeOptions.Asynchronous, 8192, 8192, security);
    }
    public static Caller Identify(NamedPipeServerStream pipe)
    {
        // Charger les dépendances avant d'entrer dans un jeton d'identification sans accès aux fichiers.
        using (var serverIdentity = WindowsIdentity.GetCurrent()) {
            _ = serverIdentity.User?.Value;
            _ = new WindowsPrincipal(serverIdentity).IsInRole(WindowsBuiltInRole.Administrator);
            _ = GetTokenInformation(serverIdentity.AccessToken.DangerousGetHandle(), 20, out _, sizeof(int), out _);
        }
        Caller? result = null;
        if (!GetNamedPipeClientSessionId(pipe.SafePipeHandle, out var session) || session == 0 || !GetNamedPipeClientProcessId(pipe.SafePipeHandle, out var pid)) throw new UnauthorizedAccessException("Session Windows interactive requise.");
        PipeStreamImpersonationWorker identify = () => {
            using var identity = WindowsIdentity.GetCurrent(true) ?? throw new UnauthorizedAccessException("Client non authentifié.");
            var admin = new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
            var full = GetTokenInformation(identity.AccessToken.DangerousGetHandle(), 20, out var elevated, sizeof(int), out _) && elevated != 0;
            result = new(identity.User!.Value, admin && full, session, pid);
        };
        RuntimeHelpers.PrepareDelegate(identify);
        pipe.RunAsClient(identify);
        return result ?? throw new UnauthorizedAccessException("Client Windows non identifié.");
    }
    public static async Task WriteAsync<T>(Stream stream, T value, CancellationToken token)
    {
        var bytes = JsonSerializer.SerializeToUtf8Bytes(value, AppPaths.Json);
        if (bytes.Length > 32 * 1024 * 1024) throw new IOException("Réponse trop volumineuse.");
        var length = new byte[4]; BinaryPrimitives.WriteInt32LittleEndian(length, bytes.Length);
        await stream.WriteAsync(length, token); await stream.WriteAsync(bytes, token); await stream.FlushAsync(token);
    }
    public static async Task<T> ReadAsync<T>(Stream stream, int maximum, CancellationToken token)
    {
        var header = new byte[4]; await stream.ReadExactlyAsync(header, token);
        var length = BinaryPrimitives.ReadInt32LittleEndian(header);
        if (length <= 0 || length > maximum) throw new IOException("Taille de requête invalide.");
        var bytes = new byte[length]; await stream.ReadExactlyAsync(bytes, token);
        return JsonSerializer.Deserialize<T>(bytes, AppPaths.Json) ?? throw new IOException("Requête invalide.");
    }
    private static uint ServicePid()
    {
        var manager = OpenSCManager(null, null, 1);
        if (manager == IntPtr.Zero) throw new IOException("Gestionnaire de services inaccessible.");
        try {
            var service = OpenService(manager, AppPaths.ServiceName, 4);
            if (service == IntPtr.Zero) throw new IOException("Service du coffre non installé.");
            try {
                if (!QueryServiceStatusEx(service, 0, out var state, Marshal.SizeOf<ServiceState>(), out _) || state.Pid == 0) throw new IOException("Service du coffre arrêté.");
                return state.Pid;
            }
            finally { CloseServiceHandle(service); }
        }
        finally { CloseServiceHandle(manager); }
    }
    public static async Task<JsonElement> SendAsync(string op, object? payload = null, string? ownerSid = null, CancellationToken token = default)
    {
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(token); timeout.CancelAfter(TimeSpan.FromMinutes(2));
        using var pipe = new NamedPipeClientStream(".", AppPaths.PipeName, PipeDirection.InOut, PipeOptions.Asynchronous, TokenImpersonationLevel.Identification);
        await pipe.ConnectAsync(3000, timeout.Token);
        if (!GetNamedPipeServerProcessId(pipe.SafePipeHandle, out var pid) || pid != ServicePid()) throw new UnauthorizedAccessException("Identité du service refusée.");
        await WriteAsync(pipe, new Request(op, AppPaths.Element(payload ?? new { }), ownerSid), timeout.Token);
        var response = await ReadAsync<Reply>(pipe, 32 * 1024 * 1024, timeout.Token);
        if (!response.Success) throw new InvalidOperationException(response.Error ?? "Opération refusée.");
        return response.Data;
    }
}
