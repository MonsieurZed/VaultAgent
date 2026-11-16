//! Adaptateur unique entre les types du moteur (`Item`, `Folder`, `Organization`) et les `serde_json::Value` que
//! lisent les projections et les contrôles de `vaultagent-core`.

use serde::Serialize;
use serde_json::{Map, Value};
use vaultagent_engine::Item;

use crate::error::Result;

/// Convertit une fiche typée vers la forme JSON du moteur Bitwarden : noms en camelCase, `null` pour les champs
/// absents, et `attachments` (tableau non vide) quand la fiche porte au moins une pièce jointe.
///
/// Paramètres : la fiche.
/// Retour : la valeur JSON lue par `vaultagent-core`.
///
/// # Errors
///
/// Erreur de sérialisation (jamais pour une fiche valide).
pub fn item_value(item: &Item) -> Result<Value> {
    let mut value = serde_json::to_value(item)?;
    if let Value::Object(map) = &mut value {
        map.remove("hasAttachments");
        if item.has_attachments {
            map.insert("attachments".to_owned(), Value::Array(vec![Value::Object(Map::new())]));
        }
    }
    Ok(value)
}

/// Convertit une liste de fiches.
///
/// Paramètres : les fiches.
/// Retour : leurs valeurs JSON, dans l'ordre.
///
/// # Errors
///
/// Erreur de sérialisation.
pub fn item_values(items: &[Item]) -> Result<Vec<Value>> {
    items.iter().map(item_value).collect()
}

/// Convertit une liste de dossiers ou d'organisations vers des valeurs JSON.
///
/// Paramètres : les éléments sérialisables.
/// Retour : leurs valeurs JSON, dans l'ordre.
///
/// # Errors
///
/// Erreur de sérialisation.
pub fn values_of<T: Serialize>(entries: &[T]) -> Result<Vec<Value>> {
    entries
        .iter()
        .map(|entry| serde_json::to_value(entry).map_err(Into::into))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vaultagent_core::{active, catalog_entry, item_detail};
    use vaultagent_engine::{Login, LoginUri, Secret};

    /// Aide de test : sample.
    fn sample() -> Item {
        Item {
            id: "00000000-0000-4000-8000-000000000001".into(),
            name: "Compte".into(),
            kind: 1,
            revision_date: "2026-11-08T00:00:00.000Z".into(),
            login: Some(Login {
                username: Some("u".into()),
                password: Some(Secret::from("fictif-1")),
                totp: None,
                uris: vec![LoginUri {
                    uri: "https://example.test/login".into(),
                }],
            }),
            ..Item::default()
        }
    }

    #[test]
    fn typed_items_feed_the_core_projections() {
        let value = item_value(&sample()).unwrap_or_default();
        let entry = catalog_entry(&value);
        assert_eq!(
            entry.as_ref().map(|e| e.origins.clone()),
            Some(vec!["https://example.test".to_owned()])
        );
        assert_eq!(
            entry.map(|e| (e.username, e.password, e.totp)),
            Some((true, true, false))
        );
        assert!(item_detail(&value).is_some_and(|d| !d.deleted && d.has.password));
        assert!(active(&value).is_ok());
    }

    #[test]
    fn attachments_become_an_array_and_the_flag_disappears() {
        let item = Item {
            has_attachments: true,
            ..sample()
        };
        let value = item_value(&item).unwrap_or_default();
        assert_eq!(
            value.get("attachments").and_then(Value::as_array).map(Vec::len),
            Some(1)
        );
        assert!(value.get("hasAttachments").is_none());
        assert!(item_value(&sample()).unwrap_or_default().get("attachments").is_none());
    }
}
