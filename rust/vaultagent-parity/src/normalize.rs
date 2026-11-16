//! Normalisation des champs variables d'une réponse (dates, identifiants, jetons, empreintes, codes) : les valeurs
//! qui changent d'une exécution ou d'un serveur à l'autre deviennent des marqueurs, les secrets ne sont jamais gardés.
//! L'ordre des listes de fiches, dossiers, collections et organisations n'est pas garanti par le protocole (Node suit
//! l'ordre de synchronisation, Rust celui de son dépôt) : ces listes sont triées avant la comparaison ; `uris` garde
//! son ordre.

use serde_json::{Map, Value};

const SECRET_KEYS: [&str; 9] = [
    "token",
    "secret",
    "signature",
    "challenge",
    "encryptedKey",
    "nonce",
    "tag",
    "data",
    "approverPublicKey",
];
const FIXED_REQUEST_IDS: [&str; 3] = ["reset-auto", "enroll-hello", "enroll-elevation"];
const NUMBER_KEYS: [&str; 3] = ["expiresInSeconds", "created", "ageSeconds"];
const UNORDERED_KEYS: [&str; 11] = [
    "",
    "items",
    "trash",
    "folders",
    "organizations",
    "collections",
    "collectionIds",
    "devices",
    "pairRequests",
    "unlockRequests",
    "approvalRequests",
];
const OPAQUE_MIN_LEN: usize = 24;

/// Indique si un texte a la forme d'un UUID.
fn is_uuid(text: &str) -> bool {
    let groups: Vec<&str> = text.split('-').collect();
    let sizes = [8, 4, 4, 4, 12];
    groups.len() == 5
        && groups
            .iter()
            .zip(sizes)
            .all(|(group, size)| group.len() == size && group.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Indique si un texte commence par une date ISO 8601 (`2026-11-16T10:00:00`).
fn is_iso_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 19
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[..4].iter().all(u8::is_ascii_digit)
}

/// Indique si un texte est un code de vérification `ABCD-EFGH`.
fn is_verification(text: &str) -> bool {
    text.len() == 9
        && text.split_once('-').is_some_and(|(left, right)| {
            [left, right]
                .iter()
                .all(|part| part.len() == 4 && part.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
        })
}

/// Indique si un texte ressemble à une valeur opaque (base64, base64url, hexadécimal) assez longue pour être secrète.
fn is_opaque(text: &str) -> bool {
    text.len() >= OPAQUE_MIN_LEN && text.chars().all(|c| c.is_ascii_alphanumeric() || "+/=_-".contains(c))
}

/// Remplace une chaîne variable par son marqueur.
///
/// Paramètres : la clé de l'objet parent (vide dans un tableau) et le texte.
/// Retour : le texte conservé ou son marqueur.
fn normalize_text(key: &str, text: &str) -> String {
    if SECRET_KEYS.contains(&key) || text.starts_with("va1.") {
        return "<secret>".to_owned();
    }
    if key == "requestId" && !FIXED_REQUEST_IDS.contains(&text) {
        return "<id>".to_owned();
    }
    if key == "code" {
        return text
            .split_once('.')
            .map_or("<code>".to_owned(), |(vault, _)| format!("{vault}.<code>"));
    }
    if is_uuid(text) {
        return "<uuid>".to_owned();
    }
    if is_iso_date(text) {
        return "<date>".to_owned();
    }
    if is_verification(text) {
        return "<verification>".to_owned();
    }
    if is_opaque(text) {
        return "<opaque>".to_owned();
    }
    text.to_owned()
}

/// Normalise une valeur JSON, récursivement.
///
/// Paramètres : la clé de l'objet parent et la valeur.
/// Retour : la valeur dont les champs variables sont remplacés par des marqueurs.
fn normalize_in(key: &str, value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(normalize_text(key, text)),
        Value::Number(_) if NUMBER_KEYS.contains(&key) => Value::String("<number>".to_owned()),
        Value::Array(items) => {
            let mut normalized: Vec<Value> = items.iter().map(|item| normalize_in("", item)).collect();
            if UNORDERED_KEYS.contains(&key) {
                normalized.sort_by_key(Value::to_string);
            }
            Value::Array(normalized)
        }
        Value::Object(fields) => {
            let mut result = Map::new();
            for (name, field) in fields {
                result.insert(name.clone(), normalize_in(name, field));
            }
            Value::Object(result)
        }
        other => other.clone(),
    }
}

/// Normalise le corps d'une réponse.
///
/// Paramètres : la valeur JSON reçue.
/// Retour : la valeur sans champ variable ni secret.
#[must_use]
pub fn normalize(value: &Value) -> Value {
    normalize_in("", value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::from_str;

    /// Analyse un texte JSON de test.
    fn parse(text: &str) -> Value {
        from_str(text).unwrap()
    }

    #[test]
    fn masks_secrets_ids_and_dates() {
        let value = parse(
            "{\"token\":\"va1.x.y\",\"id\":\"00000000-0000-4000-8000-0000000000b1\",\"expiresUtc\":\"2026-11-16T10:00:00.000Z\",\"verification\":\"AB12-CD34\"}",
        );
        let expected = parse(
            "{\"token\":\"<secret>\",\"id\":\"<uuid>\",\"expiresUtc\":\"<date>\",\"verification\":\"<verification>\"}",
        );
        assert_eq!(normalize(&value), expected);
    }

    #[test]
    fn keeps_stable_values() {
        let value =
            parse("{\"error\":\"origin-refused\",\"unlocked\":false,\"expiresUtc\":null,\"vaultId\":\"default\"}");
        assert_eq!(normalize(&value), value);
    }

    #[test]
    fn masks_random_request_ids_but_keeps_fixed_ones() {
        let value = parse("{\"requestId\":\"qK_TNN5sdAlxVkojWysJCA\",\"other\":{\"requestId\":\"reset-auto\"}}");
        assert_eq!(
            normalize(&value),
            parse("{\"requestId\":\"<id>\",\"other\":{\"requestId\":\"reset-auto\"}}")
        );
    }

    #[test]
    fn sorts_unordered_lists_but_keeps_uri_order() {
        let value = parse("{\"items\":[{\"name\":\"b\"},{\"name\":\"a\"}],\"uris\":[\"z\",\"y\"]}");
        let expected = parse("{\"items\":[{\"name\":\"a\"},{\"name\":\"b\"}],\"uris\":[\"z\",\"y\"]}");
        assert_eq!(normalize(&value), expected);
    }

    #[test]
    fn keeps_vault_prefix_of_pairing_code() {
        assert_eq!(
            normalize(&parse("{\"code\":\"default.abcDEF123\"}")),
            parse("{\"code\":\"default.<code>\"}")
        );
    }

    #[test]
    fn masks_long_opaque_strings_and_variable_numbers() {
        let value = parse("{\"list\":[\"QUJDREVGR0hJSktMTU5PUFFSU1RVVldY\"],\"expiresInSeconds\":299}");
        assert_eq!(
            normalize(&value),
            parse("{\"list\":[\"<opaque>\"],\"expiresInSeconds\":\"<number>\"}")
        );
    }
}
