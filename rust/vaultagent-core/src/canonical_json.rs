//! Sérialisation canonique octet pour octet identique à `JSON.stringify(canonical(valeur))` de Node : clés triées par
//! unités UTF-16 (indices de tableau d'abord, comme les objets JavaScript), valeurs `null` d'objet retirées, chaînes
//! échappées comme `JSON.stringify`, nombres écrits comme `Number.prototype.toString`.

use serde_json::{Number, Value};
use std::cmp::Ordering;

/// Plus grand indice de tableau JavaScript (`2^32 - 2`).
const MAX_ARRAY_INDEX: u64 = 4_294_967_294;

/// Sérialise une valeur sous forme canonique, comme `JSON.stringify(canonical(value))`.
///
/// Paramètres : la valeur JSON à sérialiser.
/// Retour : le texte JSON compact.
#[must_use]
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value);
    out
}

/// Écrit une valeur JSON à la suite du texte.
fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => out.push_str(&js_number(number)),
        Value::String(text) => write_string(out, text),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().filter(|(_, v)| !v.is_null()).collect();
            entries.sort_by(|a, b| key_order(a.0, b.0));
            out.push('{');
            for (index, (key, item)) in entries.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_value(out, item);
            }
            out.push('}');
        }
    }
}

/// Indice de tableau JavaScript représenté par une clé (`"0"`, `"17"`, sans zéro de tête), s'il y en a un.
fn array_index(key: &str) -> Option<u64> {
    let canonical_digits =
        !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit()) && (key == "0" || !key.starts_with('0'));
    canonical_digits
        .then(|| key.parse::<u64>().ok())
        .flatten()
        .filter(|index| *index <= MAX_ARRAY_INDEX)
}

/// Ordre des clés d'un objet JavaScript : indices croissants, puis les autres clés triées par unités UTF-16.
fn key_order(left: &str, right: &str) -> Ordering {
    match (array_index(left), array_index(right)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.encode_utf16().cmp(right.encode_utf16()),
    }
}

/// Écrit une chaîne entre guillemets avec l'échappement de `JSON.stringify`.
fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if control < ' ' => {
                out.push_str("\\u00");
                push_hex_byte(out, u8::try_from(u32::from(control)).unwrap_or_default());
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// Ajoute un octet en deux chiffres hexadécimaux minuscules.
pub(crate) fn push_hex_byte(out: &mut String, byte: u8) {
    for nibble in [byte >> 4, byte & 0x0f] {
        out.push(char::from_digit(u32::from(nibble), 16).unwrap_or('0'));
    }
}

/// Écrit un nombre comme JavaScript : lu comme un `f64`, `-0` devient `0`, exposant au-delà de `1e21` et sous `1e-6`.
fn js_number(number: &Number) -> String {
    let value = number.as_f64().unwrap_or(0.0);
    if value == 0.0 {
        return "0".to_owned();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let point = exponent.parse::<i64>().unwrap_or_default() + 1;
    format!("{sign}{}", place_digits(&digits, point))
}

/// Place la virgule des chiffres significatifs selon l'algorithme de `Number::toString` (`point` = exposant + 1).
fn place_digits(digits: &str, point: i64) -> String {
    let count = i64::try_from(digits.len()).unwrap_or(i64::MAX);
    let split = usize::try_from(point).unwrap_or(0);
    let zeros = |n: i64| "0".repeat(usize::try_from(n).unwrap_or(0));
    if count <= point && point <= 21 {
        format!("{digits}{}", zeros(point - count))
    } else if 0 < point && point <= 21 {
        format!("{}.{}", &digits[..split], &digits[split..])
    } else if -6 < point && point <= 0 {
        format!("0.{}{digits}", zeros(-point))
    } else {
        let exponent = point - 1;
        let sign = if exponent < 0 { '-' } else { '+' };
        let (head, tail) = digits.split_at(1);
        let fraction = if tail.is_empty() {
            String::new()
        } else {
            format!(".{tail}")
        };
        format!("{head}{fraction}e{sign}{}", exponent.abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_follow_javascript_rules() {
        let cases = [
            ("1.0", "1"),
            ("-0", "0"),
            ("1e21", "1e+21"),
            ("1e20", "100000000000000000000"),
            ("1e-7", "1e-7"),
            ("0.000001", "0.000001"),
            ("1.5", "1.5"),
            ("-12.25", "-12.25"),
        ];
        for (input, expected) in cases {
            let value: Value = serde_json::from_str(input).unwrap();
            assert_eq!(canonical_json(&value), expected, "{input}");
        }
    }

    #[test]
    fn strings_escape_like_json_stringify() {
        let text = canonical_json(&json!("a\u{2028}\u{1f}\u{7f}\"\\\n"));
        assert_eq!(text, "\"a\u{2028}\\u001f\u{7f}\\\"\\\\\\n\"");
    }

    #[test]
    fn keys_sort_by_utf16_with_indices_first() {
        let value: Value = serde_json::from_str(r#"{"b":1,"10":2,"9":3,"a":null,"":4,"😀":5}"#).unwrap();
        assert_eq!(
            canonical_json(&value),
            "{\"9\":3,\"10\":2,\"b\":1,\"\u{1f600}\":5,\"\u{e000}\":4}"
        );
    }

    #[test]
    fn array_nulls_are_kept() {
        assert_eq!(canonical_json(&json!([null, {"a": null}])), "[null,{}]");
    }
}
