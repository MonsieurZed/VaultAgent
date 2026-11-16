//! Fiche du coffre telle que le serveur la lit : métadonnées, connexion, historique et sections opaques.
//!
//! Seuls figurent les champs que `linux/*.mjs` et `vaultagent-core` lisent. Les sections `secure_note`, `card`,
//! `identity`, `ssh_key` et `fields` ne sont lues que pour comparer l'avant et l'après d'un partage.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::secret::Secret;

/// Adresse d'une connexion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginUri {
    /// Adresse enregistrée, telle quelle.
    pub uri: String,
}

/// Connexion d'une fiche de type 1.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Login {
    /// Identifiant de connexion.
    pub username: Option<String>,
    /// Mot de passe.
    pub password: Option<Secret>,
    /// Graine ou URI TOTP.
    pub totp: Option<Secret>,
    /// Adresses de la connexion.
    pub uris: Vec<LoginUri>,
}

/// Ancien mot de passe d'une fiche.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordHistoryEntry {
    /// Mot de passe remplacé.
    pub password: Secret,
    /// Date ISO du remplacement.
    pub last_used_date: Option<String>,
}

/// Fiche du coffre.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    /// Identifiant de la fiche.
    pub id: String,
    /// Nom de la fiche.
    pub name: String,
    /// Type de fiche (1 connexion, 2 note sécurisée).
    #[serde(rename = "type")]
    pub kind: i64,
    /// Dossier, ou `None`.
    pub folder_id: Option<String>,
    /// Date de révision ISO, telle que fournie par le serveur Bitwarden.
    pub revision_date: String,
    /// Organisation propriétaire.
    pub organization_id: Option<String>,
    /// Collections de la fiche.
    pub collection_ids: Vec<String>,
    /// Date de mise en corbeille.
    pub deleted_date: Option<String>,
    /// Date d'archivage.
    pub archived_date: Option<String>,
    /// Notes.
    pub notes: Option<Secret>,
    /// Connexion, pour une fiche de type 1.
    pub login: Option<Login>,
    /// La fiche porte au moins une pièce jointe.
    pub has_attachments: bool,
    /// Anciens mots de passe, le plus récent d'abord.
    pub password_history: Vec<PasswordHistoryEntry>,
    /// Section de note sécurisée.
    pub secure_note: Option<Value>,
    /// Section de carte.
    pub card: Option<Value>,
    /// Section d'identité.
    pub identity: Option<Value>,
    /// Section de clé SSH.
    pub ssh_key: Option<Value>,
    /// Champs personnalisés.
    pub fields: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_bitwarden_names_and_defaults_missing_fields() {
        let item: Item = serde_json::from_value(json!({
            "id": "i", "name": "n", "type": 1, "revisionDate": "2026-11-16T10:00:00.000Z",
            "collectionIds": ["c"], "login": {"password": "fictif-1", "uris": [{"uri": "https://a.test"}]}
        }))
        .unwrap_or_default();
        assert_eq!((item.kind, item.collection_ids.len()), (1, 1));
        assert_eq!(
            item.login
                .as_ref()
                .and_then(|l| l.password.as_ref())
                .map(Secret::expose),
            Some("fictif-1")
        );
        assert!(item.folder_id.is_none() && item.deleted_date.is_none() && !item.has_attachments);
    }

    #[test]
    fn debug_never_shows_secrets() {
        let item = Item {
            notes: Some(Secret::from("note-secrete")),
            login: Some(Login {
                password: Some(Secret::from("fake-password")),
                ..Login::default()
            }),
            ..Item::default()
        };
        let text = format!("{item:?}");
        assert!(!text.contains("note-secrete") && !text.contains("fake-password"));
    }
}
