//! Petites règles de lecture JSON alignées sur la sémantique de Node (vérité, longueur UTF-16, espaces, clés).

use serde_json::{Map, Value};

/// Vérité à la manière de JavaScript : `null`, `false`, `0` et la chaîne vide sont faux, le reste est vrai.
#[must_use]
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// Vérité d'un champ optionnel d'objet : absent est faux.
#[must_use]
pub fn field_truthy(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).is_some_and(truthy)
}

/// Longueur en unités UTF-16, comme `String.length` de JavaScript (les bornes du protocole sont dans cette unité).
#[must_use]
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Indique si un caractère est un espace au sens de `String.prototype.trim`.
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
    ) || matches!(
        c,
        '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

/// Indique si un texte est vide une fois les espaces JavaScript retirés.
#[must_use]
pub fn is_blank(text: &str) -> bool {
    text.chars().all(is_js_space)
}

/// Retire les espaces JavaScript en début et en fin de texte, comme `String.prototype.trim`.
#[must_use]
pub fn trim(text: &str) -> &str {
    text.trim_matches(is_js_space)
}

/// Indique si un texte contient un caractère de contrôle `U+0000` à `U+001F`.
#[must_use]
pub fn has_control(text: &str) -> bool {
    text.chars().any(|c| c < ' ')
}

/// Lit un nombre JSON entier, y compris écrit `1.0`.
#[must_use]
pub fn small_int(value: &Value) -> Option<i64> {
    let number = value.as_number()?;
    number.as_i64().or_else(|| {
        let float = number.as_f64()?;
        let whole = float.fract() == 0.0 && float.abs() < 1e15;
        #[allow(clippy::cast_possible_truncation)]
        whole.then_some(float as i64)
    })
}

/// Construit un objet JSON à partir de paires clé-valeur.
#[must_use]
pub fn object(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(pairs.into_iter().map(|(key, value)| (key.to_owned(), value)).collect())
}

/// Rend l'objet si c'est un objet simple dont toutes les clés sont autorisées (équivalent de `keys` de Node).
#[must_use]
pub fn only_keys<'a>(value: &'a Value, allowed: &[&str]) -> Option<&'a Map<String, Value>> {
    let map = value.as_object()?;
    map.keys().all(|key| allowed.contains(&key.as_str())).then_some(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn truthiness_matches_javascript() {
        for falsy in [json!(null), json!(false), json!(0), json!(""), json!(0.0)] {
            assert!(!truthy(&falsy), "{falsy}");
        }
        for value in [json!(true), json!(1), json!("x"), json!([]), json!({})] {
            assert!(truthy(&value), "{value}");
        }
    }

    #[test]
    fn utf16_length_counts_surrogates() {
        assert_eq!(utf16_len("ab"), 2);
        assert_eq!(utf16_len("\u{1f600}"), 2);
    }

    #[test]
    fn blank_and_control_detection() {
        assert!(is_blank(" \u{a0}\u{feff}\t"));
        assert!(!is_blank(" a "));
        assert!(has_control("a\u{1f}"));
        assert!(!has_control("a\u{7f}é"));
    }

    #[test]
    fn small_int_accepts_float_notation() {
        assert_eq!(small_int(&json!(1)), Some(1));
        assert_eq!(small_int(&json!(2.0)), Some(2));
        assert_eq!(small_int(&json!(1.5)), None);
        assert_eq!(small_int(&json!("1")), None);
    }

    #[test]
    fn only_keys_requires_plain_object() {
        assert!(only_keys(&json!({"a": 1}), &["a", "b"]).is_some());
        assert!(only_keys(&json!({"c": 1}), &["a"]).is_none());
        assert!(only_keys(&json!([1]), &["a"]).is_none());
        assert!(only_keys(&json!(null), &["a"]).is_none());
    }
}
