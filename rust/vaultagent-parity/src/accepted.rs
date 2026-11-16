//! Écarts acceptés entre le serveur Node et le serveur Rust : chacun est une différence voulue ou prévue au plan,
//! avec sa raison. Un écart absent de cette liste fait échouer l'outil.

use crate::diff::Mismatch;

/// Un écart accepté, repéré par l'étape et le chemin (préfixe).
#[derive(Debug)]
pub struct Accepted {
    /// Nom de l'étape, ou `*` pour toutes.
    pub label: &'static str,
    /// Préfixe du chemin de la valeur.
    pub path: &'static str,
    /// Raison de l'acceptation.
    pub reason: &'static str,
}

/// Liste des écarts acceptés.
pub const ACCEPTED: &[Accepted] = &[
    Accepted {
        label: "agent-write-preview-list",
        path: "body.folders",
        reason: "Node ajoute le faux dossier {id:\"\", name:\"No Folder\"} de la CLI Bitwarden ; Rust ne liste que les vrais dossiers",
    },
    Accepted {
        label: "agent-write-preview-trash",
        path: "body.folders",
        reason: "Node ajoute le faux dossier {id:\"\", name:\"No Folder\"} de la CLI Bitwarden ; Rust ne liste que les vrais dossiers",
    },
];

/// Cherche l'entrée de la liste qui couvre un écart.
///
/// Paramètres : la liste acceptée et l'écart.
/// Retour : l'entrée, si l'écart est accepté.
fn covering<'a>(list: &'a [Accepted], mismatch: &Mismatch) -> Option<&'a Accepted> {
    list.iter()
        .find(|entry| (entry.label == "*" || entry.label == mismatch.label) && mismatch.path.starts_with(entry.path))
}

/// Sépare les écarts refusés des écarts acceptés.
///
/// Paramètres : la liste des écarts acceptés et les écarts trouvés.
/// Retour : les écarts refusés, puis les écarts acceptés avec leur raison.
#[must_use]
pub fn split(list: &[Accepted], mismatches: Vec<Mismatch>) -> (Vec<Mismatch>, Vec<(Mismatch, &str)>) {
    let mut refused = Vec::new();
    let mut accepted = Vec::new();
    for mismatch in mismatches {
        match covering(list, &mismatch) {
            Some(entry) => accepted.push((mismatch, entry.reason)),
            None => refused.push(mismatch),
        }
    }
    (refused, accepted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un écart sans valeurs pour les tests.
    fn mismatch(label: &str, path: &str) -> Mismatch {
        Mismatch {
            label: label.to_owned(),
            path: path.to_owned(),
            node: String::new(),
            rust: String::new(),
        }
    }

    #[test]
    fn splits_by_label_and_path_prefix() {
        let list = [Accepted {
            label: "front",
            path: "body",
            reason: "front différent",
        }];
        let found = vec![
            mismatch("front", "body.x"),
            mismatch("status", "body.x"),
            mismatch("front", "status"),
        ];
        let (refused, accepted) = split(&list, found);
        assert_eq!(refused.len(), 2);
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].1, "front différent");
    }
}
