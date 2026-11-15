//! Forme canonique d'une valeur JSON, base de l'empreinte d'idempotence : clés triées, valeurs `null` retirées.

use serde_json::{Map, Value};

/// Met une valeur sous forme canonique, récursivement.
///
/// Les objets perdent leurs clés à valeur `null` et sont reconstruits clés triées par unités UTF-16 (ordre de
/// `Array.sort` de Node) ; les tableaux gardent leur ordre et leurs `null`.
///
/// Paramètres : la valeur à normaliser.
/// Retour : une copie normalisée.
#[must_use]
pub fn canonical(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().filter(|(_, v)| !v.is_null()).collect();
            entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, v)| (key.clone(), canonical(v)))
                    .collect::<Map<_, _>>(),
            )
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn removes_nulls_recursively_and_keeps_array_nulls() {
        let input = json!({"b": null, "a": {"x": null, "y": 1}, "c": [null, {"z": null, "w": 2}]});
        assert_eq!(canonical(&input), json!({"a": {"y": 1}, "c": [null, {"w": 2}]}));
    }

    #[test]
    fn key_order_does_not_change_serialization() {
        let first = canonical(&json!({"b": 1, "a": 2}));
        let second = canonical(&json!({"a": 2, "b": 1}));
        assert_eq!(first.to_string(), second.to_string());
    }

    #[test]
    fn scalars_are_unchanged() {
        assert_eq!(canonical(&json!("x")), json!("x"));
        assert_eq!(canonical(&json!(null)), json!(null));
    }
}
