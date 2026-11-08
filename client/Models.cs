using System.Text.Json;
namespace Homelab.Vault;
internal static class AppPaths
{
    public static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);
}
