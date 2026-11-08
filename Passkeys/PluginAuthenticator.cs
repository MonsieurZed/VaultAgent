using System.Runtime.InteropServices;

namespace Homelab.Vault.Passkeys;

[ComVisible(true), ClassInterface(ClassInterfaceType.None)]
public sealed class PluginAuthenticator : IPluginAuthenticator
{
    private readonly Control dispatcher;
    private int active;
    private Guid transaction;
    private PasskeyOperationForm? form;
    internal PluginAuthenticator(Control dispatcher) { this.dispatcher = dispatcher; }
    public int MakeCredential(in PluginRequest request, out PluginResponse response) => Execute(request, true, out response);
    public int GetAssertion(in PluginRequest request, out PluginResponse response) => Execute(request, false, out response);
    private int Execute(PluginRequest request, bool create, out PluginResponse response)
    {
        response = default;
        if (Interlocked.CompareExchange(ref active, 1, 0) != 0) return unchecked((int)0x800700AA);
        try {
            if (request.RequestType != 1) return unchecked((int)0x80090029);
            var message = new SignedPasskeyRequest(request.TransactionId, create, WindowsPasskeyApi.Copy(request.Data, request.DataLength), WindowsPasskeyApi.Copy(request.Signature, request.SignatureLength));
            byte[]? result = null;
            void Invoke() {
                transaction = request.TransactionId;
                using var operation = new PasskeyOperationForm(message); form = operation;
                try { operation.ShowDialog(); result = operation.Response; } finally { form = null; transaction = Guid.Empty; }
            }
            if (dispatcher.InvokeRequired) dispatcher.Invoke(Invoke); else Invoke();
            if (result == null) return unchecked((int)0x80090036);
            response.Length = (uint)result.Length; response.Data = Marshal.AllocCoTaskMem(result.Length); Marshal.Copy(result, 0, response.Data, result.Length); return 0;
        }
        catch { return unchecked((int)0x80090020); }
        finally { Interlocked.Exchange(ref active, 0); }
    }
    public int CancelOperation(in PluginCancel request) {
        var id = request.TransactionId;
        dispatcher.BeginInvoke(() => { if (transaction == id) form?.Close(); }); return 0;
    }
    public int GetLockStatus(out uint status) {
        status = 0;
        try { using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(2)); var state = PipeTransport.SendAsync("status", token: timeout.Token).GetAwaiter().GetResult(); status = state.GetProperty("unlocked").GetBoolean() ? 1u : 0; } catch { }
        return 0;
    }
}
