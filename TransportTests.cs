namespace Homelab.Vault;

internal static class TransportTests
{
    private sealed class NoPumpContext : SynchronizationContext
    {
        public override void Post(SendOrPostCallback callback, object? state) { }
    }
    private sealed class AsyncStream : MemoryStream
    {
        public override async ValueTask WriteAsync(ReadOnlyMemory<byte> value, CancellationToken token = default) {
            await Task.Delay(5, token).ConfigureAwait(false); base.Write(value.Span);
        }
        public override async ValueTask<int> ReadAsync(Memory<byte> value, CancellationToken token = default) {
            await Task.Delay(5, token).ConfigureAwait(false); return base.Read(value.Span);
        }
    }
    public static void Run()
    {
        var previous = SynchronizationContext.Current;
        using var stream = new AsyncStream();
        try {
            SynchronizationContext.SetSynchronizationContext(new NoPumpContext());
            var write = PipeTransport.WriteAsync(stream, new Request("status", AppPaths.Element(new { })), CancellationToken.None);
            if (!write.Wait(TimeSpan.FromSeconds(2))) throw new Exception("IPC bloque par le contexte graphique a l ecriture.");
            stream.Position = 0;
            var read = PipeTransport.ReadAsync<Request>(stream, 1024, CancellationToken.None);
            if (!read.Wait(TimeSpan.FromSeconds(2)) || read.Result.Op != "status") throw new Exception("IPC bloque par le contexte graphique a la lecture.");
        }
        finally { SynchronizationContext.SetSynchronizationContext(previous); }
    }
}
