//! Dossiers, organisations et collections : ce que `writes.mjs` et `sharing.mjs` lisent de la structure du coffre.

use serde::{Deserialize, Serialize};

/// Dossier personnel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Identifiant du dossier.
    pub id: String,
    /// Nom du dossier.
    pub name: String,
}

/// Collection d'une organisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    /// Identifiant de la collection.
    pub id: String,
    /// Nom de la collection.
    pub name: String,
    /// L'utilisateur peut y écrire des fiches.
    pub can_edit: bool,
}

/// Organisation active de l'utilisateur, avec ses collections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    /// Identifiant de l'organisation.
    pub id: String,
    /// Nom de l'organisation.
    pub name: String,
    /// Rôle de l'utilisateur (type d'organisation Bitwarden).
    pub role: i64,
    /// Collections de l'organisation.
    pub collections: Vec<Collection>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn organization_uses_the_protocol_names() {
        let value = serde_json::to_value(Organization {
            id: "o".into(),
            name: "Org".into(),
            role: 0,
            collections: vec![Collection {
                id: "c".into(),
                name: "Col".into(),
                can_edit: true,
            }],
        })
        .unwrap_or_default();
        assert_eq!(
            value,
            json!({"id": "o", "name": "Org", "role": 0, "collections": [{"id": "c", "name": "Col", "canEdit": true}]})
        );
    }
}
