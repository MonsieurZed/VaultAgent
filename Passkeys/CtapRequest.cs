using System.Formats.Cbor;
using System.Globalization;

namespace Homelab.Vault.Passkeys;

internal sealed record PasskeyRequest(string RpId, string RpName, byte[] ClientHash, byte[]? UserId,
    string? UserName, string? DisplayName, IReadOnlyList<byte[]> Credentials, bool Discoverable, bool RequireVerification);

internal static class CtapRequest
{
    public static PasskeyRequest Read(byte[] encoded, bool create)
    {
        if (encoded.Length is 0 or > 65536) throw new InvalidOperationException("Requête CTAP trop volumineuse.");
        var reader = new CborReader(encoded, CborConformanceMode.Ctap2Canonical);
        var count = reader.ReadStartMap() ?? throw new InvalidOperationException("Carte CTAP définie requise.");
        if (count > 16) throw new InvalidOperationException("Trop de paramètres CTAP.");
        string? rp = null, rpName = null, userName = null, displayName = null; byte[]? hash = null, user = null;
        var ids = new List<byte[]>(); var algorithms = false; var resident = false; var verify = false;
        for (var index = 0; index < count; index++) {
            var field = reader.ReadInt32();
            if (create && field == 1 || !create && field == 2) hash = reader.ReadByteString();
            else if (create && field == 2) {
                ReadMap(reader, (key, value) => { if (key == "id") rp = Text(value); else if (key == "name") rpName = Text(value); else value.SkipValue(); });
            } else if (!create && field == 1) rp = Text(reader);
            else if (create && field == 3) {
                ReadMap(reader, (key, value) => { if (key == "id") user = value.ReadByteString(); else if (key == "name") userName = Text(value); else if (key == "displayName") displayName = Text(value); else value.SkipValue(); });
            } else if (create && field == 4) {
                var size = reader.ReadStartArray() ?? throw new InvalidOperationException("Liste d'algorithmes invalide.");
                if (size > 32) throw new InvalidOperationException("Trop d'algorithmes.");
                for (var i = 0; i < size; i++) {
                    int? alg = null; string? type = null;
                    ReadMap(reader, (key, value) => { if (key == "alg") alg = value.ReadInt32(); else if (key == "type") type = Text(value); else value.SkipValue(); });
                    algorithms |= alg == -7 && type == "public-key";
                }
                reader.ReadEndArray();
            } else if (create && field == 5 || !create && field == 3) {
                var size = reader.ReadStartArray() ?? throw new InvalidOperationException("Liste de credentials invalide.");
                if (size > 128) throw new InvalidOperationException("Trop de credentials.");
                for (var i = 0; i < size; i++) {
                    byte[]? id = null; string? type = null;
                    ReadMap(reader, (key, value) => { if (key == "id") id = value.ReadByteString(); else if (key == "type") type = Text(value); else value.SkipValue(); });
                    if (id is { Length: > 0 and <= 1024 } && type == "public-key") ids.Add(id);
                }
                reader.ReadEndArray();
            } else if (create && field == 7 || !create && field == 5) {
                ReadMap(reader, (key, value) => { if (key == "rk") resident = value.ReadBoolean(); else if (key == "uv") verify = value.ReadBoolean(); else if (key == "up") { if (!value.ReadBoolean()) throw new InvalidOperationException("Présence utilisateur requise."); } else value.SkipValue(); });
            } else if (create && field == 6 || !create && field == 4) {
                ReadMap(reader, (key, value) => {
                    if (key == "credProtect") { var policy = value.ReadInt32(); if (policy is < 1 or > 3) throw new InvalidOperationException("Politique credProtect invalide."); verify |= policy == 3; }
                    else if (key is "hmac-secret" or "prf" or "largeBlobKey" or "credBlob") throw new InvalidOperationException("Extension passkey non prise en charge.");
                    else value.SkipValue();
                });
            } else reader.SkipValue();
        }
        reader.ReadEndMap();
        if (reader.BytesRemaining != 0 || hash is not { Length: 32 } || string.IsNullOrWhiteSpace(rp)) throw new InvalidOperationException("Requête CTAP incomplète.");
        var normalized = new IdnMapping().GetAscii(rp).ToLowerInvariant();
        if (normalized.Length > 253 || rp.Contains('/') || rp.Contains(':') || Uri.CheckHostName(normalized) == UriHostNameType.Unknown || normalized != rp.ToLowerInvariant()) throw new InvalidOperationException("RP ID invalide.");
        if (create && (!algorithms || user is not { Length: > 0 and <= 64 })) throw new InvalidOperationException("Utilisateur ou algorithme passkey non pris en charge.");
        return new(normalized, rpName ?? normalized, hash, user, userName, displayName, ids, resident, verify);
    }
    private static string Text(CborReader reader) {
        var text = reader.ReadTextString(); if (text.Length > 512 || text.Any(char.IsControl)) throw new InvalidOperationException("Texte CTAP invalide."); return text;
    }
    private static void ReadMap(CborReader reader, Action<string, CborReader> read) {
        var count = reader.ReadStartMap() ?? throw new InvalidOperationException("Carte CTAP invalide.");
        if (count > 32) throw new InvalidOperationException("Carte CTAP trop volumineuse.");
        for (var i = 0; i < count; i++) read(reader.ReadTextString(), reader);
        reader.ReadEndMap();
    }
}
