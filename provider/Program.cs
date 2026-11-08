using System.Reflection;

internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        try {
            var application = Assembly.LoadFrom(Path.Combine(AppContext.BaseDirectory, "VaultAgent.dll"));
            return (int)(application.EntryPoint!.Invoke(null, new object[] { args }) ?? 1);
        }
        catch { return 1; }
    }
}
