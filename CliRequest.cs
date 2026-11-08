using System.Text;
using System.Text.Json;

namespace Homelab.Vault;

internal sealed record EngineCall(string Method, string Path, object? Body = null)
{
    public object Message() => new { op = "call", method = Method, path = Path, body = Body };
}
internal static class CliRequest
{
    public static EngineCall Parse(string[] args)
    {
        if (args.Length == 0 || args.Length > 64 || args.Sum(s => s.Length) > 60000) throw new ArgumentException("Commande invalide.");
        var verb = args[0];
        int used;
        EngineCall call;
        string Part(int index) => args.Length > index ? Uri.EscapeDataString(args[index]) : throw new ArgumentException("Argument manquant.");
        object Body(int index) {
            if (args.Length <= index) throw new ArgumentException("Objet encodé manquant.");
            return JsonSerializer.Deserialize<JsonElement>(Encoding.UTF8.GetString(Convert.FromBase64String(args[index])));
        }
        bool SafeObject(int index, params string[] values) => args.Length > index && values.Contains(args[index]);
        switch (verb) {
            case "status": used = 1; call = new("GET", "/status"); break;
            case "sync": used = 1; call = new("POST", "/sync"); break;
            case "generate": used = 1; call = new("GET", "/generate"); break;
            case "list":
                if (!SafeObject(1, "items", "folders", "collections", "organizations", "org-collections")) throw new ArgumentException("Type de liste indisponible.");
                used = 2; call = new("GET", "/list/object/" + Part(1)); break;
            case "get":
                if (!SafeObject(1, "item", "folder", "collection", "organization", "username", "password", "totp", "uri", "notes", "template", "fingerprint")) throw new ArgumentException("Accès aux fichiers refusé.");
                used = 3; call = new("GET", "/object/" + Part(1) + "/" + Part(2)); break;
            case "create":
                if (!SafeObject(1, "item", "folder")) throw new ArgumentException("Type de création indisponible.");
                used = 3; call = new("POST", "/object/" + Part(1), Body(2)); break;
            case "edit":
                if (!SafeObject(1, "item", "folder")) throw new ArgumentException("Type de modification indisponible.");
                used = 4; call = new("PUT", "/object/" + Part(1) + "/" + Part(2), Body(3)); break;
            case "delete":
                if (!SafeObject(1, "item", "folder")) throw new ArgumentException("Type de suppression indisponible.");
                used = 3; call = new("DELETE", "/object/" + Part(1) + "/" + Part(2)); break;
            case "restore":
                if (!SafeObject(1, "item")) throw new ArgumentException("Type de restauration indisponible.");
                used = 3; call = new("POST", "/restore/item/" + Part(2)); break;
            default: throw new ArgumentException("Commande indisponible via le service.");
        }
        var query = new List<string>();
        var names = new HashSet<string> { "search", "url", "folderid", "collectionid", "organizationid", "trash", "force", "last", "length", "uppercase", "lowercase", "number", "special", "passphrase", "words", "separator", "includeNumber", "capitalize" };
        for (var index = used; index < args.Length; index++) {
            if (args[index] == "--raw") continue;
            if (!args[index].StartsWith("--") || !names.Contains(args[index][2..])) throw new ArgumentException("Option indisponible via le service.");
            var name = args[index][2..];
            var value = index + 1 < args.Length && !args[index + 1].StartsWith("--") ? args[++index] : "true";
            query.Add(Uri.EscapeDataString(name) + "=" + Uri.EscapeDataString(value));
        }
        return call with { Path = call.Path + (query.Count > 0 ? "?" + string.Join("&", query) : "") };
    }
}
