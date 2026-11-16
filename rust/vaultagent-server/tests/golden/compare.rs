//! Comparaison d'un corps attendu et d'un corps reçu : inclusion des clés, valeurs génériques `{{uuid}}`,
//! `{{iso-date}}`, `{{base64}}`, `{{string}}`, `{{number}}`, y compris au milieu d'une chaîne.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::Value;

/// Classe de caractères d'une valeur générique.
fn accepts(generic: &str, text: &str) -> bool {
    match generic {
        "uuid" => {
            text.len() == 36
                && text.char_indices().all(|(index, ch)| match index {
                    8 | 13 | 18 | 23 => ch == '-',
                    _ => ch.is_ascii_hexdigit(),
                })
        }
        "iso-date" => {
            let bytes = text.as_bytes();
            bytes.len() >= 20
                && text.ends_with('Z')
                && bytes[4] == b'-'
                && bytes[10] == b'T'
                && bytes[..4].iter().all(u8::is_ascii_digit)
        }
        "base64" => {
            !text.is_empty()
                && text
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || "+/=_-".contains(ch))
        }
        "string" => !text.is_empty(),
        _ => false,
    }
}

/// Découpe `a{{x}}b` en morceaux littéraux et génériques.
fn pieces(expected: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut rest = expected;
    while let Some(start) = rest.find("{{") {
        let Some(end) = rest[start..].find("}}") else { break };
        if start > 0 {
            out.push((false, rest[..start].to_owned()));
        }
        out.push((true, rest[start + 2..start + end].to_owned()));
        rest = &rest[start + end + 2..];
    }
    if !rest.is_empty() {
        out.push((false, rest.to_owned()));
    }
    out
}

/// Compare une chaîne attendue (éventuellement générique) à une chaîne reçue.
fn string_matches(expected: &str, actual: &str) -> bool {
    let parts = pieces(expected);
    let mut cursor = actual;
    let mut index = 0;
    while index < parts.len() {
        let (generic, text) = &parts[index];
        if *generic {
            let next = parts.get(index + 1).filter(|(is_generic, _)| !is_generic);
            let take = match next {
                Some((_, literal)) => cursor.find(literal.as_str()),
                None => Some(cursor.len()),
            };
            let Some(take) = take else { return false };
            if !accepts(text, &cursor[..take]) {
                return false;
            }
            cursor = &cursor[take..];
        } else {
            let Some(after) = cursor.strip_prefix(text.as_str()) else {
                return false;
            };
            cursor = after;
        }
        index += 1;
    }
    cursor.is_empty()
}

/// Compare une valeur attendue à la valeur reçue ; rend le chemin et la raison du premier écart.
pub fn compare(expected: &Value, actual: &Value, path: &str) -> Result<(), String> {
    match (expected, actual) {
        (Value::String(want), _) if want == "{{number}}" => actual
            .is_number()
            .then_some(())
            .ok_or_else(|| format!("{path}: nombre attendu, reçu {actual}")),
        (Value::String(want), Value::String(got)) => string_matches(want, got)
            .then_some(())
            .ok_or_else(|| format!("{path}: attendu {want:?}, reçu {got:?}")),
        (Value::Object(want), Value::Object(got)) => want.iter().try_for_each(|(key, value)| {
            let child = got.get(key).ok_or_else(|| format!("{path}.{key}: clé absente"))?;
            compare(value, child, &format!("{path}.{key}"))
        }),
        (Value::Array(want), Value::Array(got)) => {
            if want.len() != got.len() {
                return Err(format!("{path}: {} éléments attendus, {} reçus", want.len(), got.len()));
            }
            want.iter()
                .zip(got)
                .enumerate()
                .try_for_each(|(index, (left, right))| compare(left, right, &format!("{path}[{index}]")))
        }
        _ => (expected == actual)
            .then_some(())
            .ok_or_else(|| format!("{path}: attendu {expected}, reçu {actual}")),
    }
}
