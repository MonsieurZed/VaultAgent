//! Rapport de comparaison : écarts refusés, écarts acceptés et résumé, sans aucune valeur secrète.

use crate::diff::Mismatch;
use crate::exchange::Exchange;

const ORDER_RULE: &str =
    "Règle : l'ordre des listes de fiches, dossiers, collections et organisations n'est pas comparé.\n";

/// Résultat chiffré d'une comparaison.
#[derive(Debug, PartialEq, Eq)]
pub struct Summary {
    /// Nombre d'échanges comparés.
    pub compared: usize,
    /// Nombre d'écarts refusés.
    pub refused: usize,
    /// Nombre d'écarts acceptés.
    pub accepted: usize,
}

/// Écrit un écart sur trois lignes.
///
/// Paramètres : l'écart.
/// Retour : le texte.
fn describe(mismatch: &Mismatch) -> String {
    format!(
        "  {} {}\n      node : {}\n      rust : {}\n",
        mismatch.label, mismatch.path, mismatch.node, mismatch.rust
    )
}

/// Code d'erreur public d'un échange (`error`), vide s'il n'y en a pas.
fn error_code(exchange: &Exchange) -> String {
    exchange
        .body
        .get("error")
        .and_then(|code| code.as_str())
        .unwrap_or_default()
        .to_owned()
}

/// Liste l'étape et le statut de chaque échange des deux serveurs, sans corps.
///
/// Paramètres : les échanges Node et Rust.
/// Retour : une ligne par étape.
#[must_use]
pub fn trace(node: &[Exchange], rust: &[Exchange]) -> String {
    let lines: Vec<String> = node
        .iter()
        .zip(rust)
        .map(|(left, right)| {
            format!(
                "  {:<34} node {:>3}  rust {:>3}  {} | {}\n",
                left.label,
                left.status,
                right.status,
                error_code(left),
                error_code(right)
            )
        })
        .collect();
    lines.concat()
}

/// Compose le rapport complet.
///
/// Paramètres : les écarts refusés, les écarts acceptés avec leur raison et le nombre d'échanges comparés.
/// Retour : le texte du rapport et le résumé.
#[must_use]
pub fn render(refused: &[Mismatch], accepted: &[(Mismatch, &str)], compared: usize) -> (String, Summary) {
    let mut parts: Vec<String> = Vec::new();
    if !refused.is_empty() {
        parts.push("Écarts refusés :\n".to_owned());
        parts.extend(refused.iter().map(describe));
    }
    if !accepted.is_empty() {
        parts.push("Écarts acceptés :\n".to_owned());
        parts.extend(
            accepted
                .iter()
                .map(|(mismatch, reason)| format!("{}      raison : {reason}\n", describe(mismatch))),
        );
    }
    parts.push(ORDER_RULE.to_owned());
    let summary = Summary {
        compared,
        refused: refused.len(),
        accepted: accepted.len(),
    };
    parts.push(format!(
        "Parité : {} requêtes comparées, {} écarts, {} écarts acceptés\n",
        summary.compared, summary.refused, summary.accepted
    ));
    (parts.concat(), summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_counts_and_text() {
        let found = Mismatch {
            label: "a".to_owned(),
            path: "status".to_owned(),
            node: "200".to_owned(),
            rust: "400".to_owned(),
        };
        let (text, summary) = render(&[found], &[], 5);
        assert_eq!(
            summary,
            Summary {
                compared: 5,
                refused: 1,
                accepted: 0
            }
        );
        assert!(text.contains("node : 200"));
        assert!(text.ends_with("5 requêtes comparées, 1 écarts, 0 écarts acceptés\n"));
    }

    #[test]
    fn clean_run_has_rule_and_summary_line() {
        let (text, summary) = render(&[], &[], 3);
        assert_eq!(summary.refused, 0);
        assert_eq!(
            text,
            format!("{ORDER_RULE}Parité : 3 requêtes comparées, 0 écarts, 0 écarts acceptés\n")
        );
    }
}
