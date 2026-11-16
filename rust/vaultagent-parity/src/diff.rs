//! Comparaison de deux séries d'échanges : statut, en-têtes utiles et corps normalisé, écart par écart.

use crate::exchange::Exchange;
use crate::normalize::normalize;
use serde_json::Value;

const SHOWN_LEN: usize = 90;

/// Un écart entre le serveur Node et le serveur Rust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    /// Étape du scénario (nom).
    pub label: String,
    /// Chemin de la valeur : `status`, `header.<nom>`, `body` puis `.clé` ou `[n]`.
    pub path: String,
    /// Valeur côté Node, normalisée et tronquée.
    pub node: String,
    /// Valeur côté Rust, normalisée et tronquée.
    pub rust: String,
}

/// Rend une valeur normalisée pour l'affichage, tronquée.
///
/// Paramètres : la valeur, ou `None` si elle est absente.
/// Retour : le texte court.
fn shown(value: Option<&Value>) -> String {
    let text = value.map_or("<absent>".to_owned(), Value::to_string);
    if text.chars().count() > SHOWN_LEN {
        let cut: String = text.chars().take(SHOWN_LEN).collect();
        format!("{cut}…")
    } else {
        text
    }
}

/// Ajoute un écart à la liste.
///
/// Paramètres : la liste, l'étape, le chemin et les deux valeurs.
/// Retour : rien.
fn push(out: &mut Vec<Mismatch>, label: &str, path: &str, node: Option<&Value>, rust: Option<&Value>) {
    out.push(Mismatch {
        label: label.to_owned(),
        path: path.to_owned(),
        node: shown(node),
        rust: shown(rust),
    });
}

/// Compare deux valeurs normalisées, récursivement.
///
/// Paramètres : la liste d'écarts, l'étape, le chemin courant, la valeur Node et la valeur Rust.
/// Retour : rien.
fn diff_values(out: &mut Vec<Mismatch>, label: &str, path: &str, node: &Value, rust: &Value) {
    match (node, rust) {
        (Value::Object(left), Value::Object(right)) => {
            for (key, value) in left {
                let child = format!("{path}.{key}");
                match right.get(key) {
                    Some(other) => diff_values(out, label, &child, value, other),
                    None => push(out, label, &child, Some(value), None),
                }
            }
            for (key, value) in right.iter().filter(|(key, _)| !left.contains_key(*key)) {
                push(out, label, &format!("{path}.{key}"), None, Some(value));
            }
        }
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
            for (index, (value, other)) in left.iter().zip(right).enumerate() {
                diff_values(out, label, &format!("{path}[{index}]"), value, other);
            }
        }
        _ if node == rust => {}
        _ => push(out, label, path, Some(node), Some(rust)),
    }
}

/// Compare deux échanges de même étape.
///
/// Paramètres : l'échange Node et l'échange Rust.
/// Retour : les écarts, vide si les deux concordent.
#[must_use]
pub fn compare_exchange(node: &Exchange, rust: &Exchange) -> Vec<Mismatch> {
    let mut out = Vec::new();
    let label = node.label.as_str();
    if node.status != rust.status {
        push(
            &mut out,
            label,
            "status",
            Some(&Value::from(node.status)),
            Some(&Value::from(rust.status)),
        );
    }
    for (name, value) in &node.headers {
        let other = rust
            .headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, text)| Value::from(text.as_str()));
        if other.as_ref() != Some(&Value::from(value.as_str())) {
            push(
                &mut out,
                label,
                &format!("header.{name}"),
                Some(&Value::from(value.as_str())),
                other.as_ref(),
            );
        }
    }
    for (name, value) in rust
        .headers
        .iter()
        .filter(|(name, _)| !node.headers.iter().any(|(key, _)| key == name))
    {
        push(
            &mut out,
            label,
            &format!("header.{name}"),
            None,
            Some(&Value::from(value.as_str())),
        );
    }
    diff_values(&mut out, label, "body", &normalize(&node.body), &normalize(&rust.body));
    out
}

/// Compare deux séries d'échanges étape par étape.
///
/// Paramètres : les échanges Node et Rust, dans l'ordre du scénario.
/// Retour : tous les écarts ; une étape sans vis-à-vis ou de nom différent est un écart de chemin `step`.
#[must_use]
pub fn compare(node: &[Exchange], rust: &[Exchange]) -> Vec<Mismatch> {
    let mut out = Vec::new();
    for index in 0..node.len().max(rust.len()) {
        match (node.get(index), rust.get(index)) {
            (Some(left), Some(right)) if left.label == right.label => out.extend(compare_exchange(left, right)),
            (left, right) => {
                let name = |exchange: Option<&Exchange>| exchange.map(|e| Value::from(e.label.as_str()));
                push(
                    &mut out,
                    &format!("#{index}"),
                    "step",
                    name(left).as_ref(),
                    name(right).as_ref(),
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::from_str;

    /// Construit un échange de test.
    fn exchange(label: &str, status: u16, body: &str) -> Exchange {
        Exchange {
            label: label.to_owned(),
            status,
            headers: vec![],
            body: from_str(body).unwrap(),
        }
    }

    #[test]
    fn equal_after_normalization() {
        let node = exchange("a", 200, "{\"id\":\"00000000-0000-4000-8000-000000000001\",\"x\":1}");
        let rust = exchange("a", 200, "{\"id\":\"11111111-1111-4111-8111-111111111111\",\"x\":1}");
        assert_eq!(compare_exchange(&node, &rust), Vec::new());
    }

    #[test]
    fn reports_status_value_and_missing_key() {
        let node = exchange("a", 403, "{\"error\":\"unauthorized\",\"only\":true}");
        let rust = exchange("a", 400, "{\"error\":\"request-refused\"}");
        let paths: Vec<String> = compare_exchange(&node, &rust).into_iter().map(|m| m.path).collect();
        assert_eq!(paths, vec!["status", "body.error", "body.only"]);
    }

    #[test]
    fn secrets_never_reach_the_diff() {
        let node = exchange("a", 200, "{\"token\":\"va1.secret-node-value\"}");
        let rust = exchange("a", 200, "{\"token\":null}");
        let text = format!("{:?}", compare_exchange(&node, &rust));
        assert!(!text.contains("secret-node-value"));
    }

    #[test]
    fn length_or_label_difference_is_reported() {
        let node = vec![exchange("a", 200, "{}"), exchange("b", 200, "{}")];
        let rust = vec![exchange("a", 200, "{}")];
        assert_eq!(compare(&node, &rust).len(), 1);
    }
}
