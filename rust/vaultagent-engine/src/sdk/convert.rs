//! Conversion d'une fiche déchiffrée du SDK (`CipherView`) vers l'`Item` du moteur ; les dates sortent en ISO 8601
//! UTC, les sections opaques en JSON du SDK (elles ne servent qu'à comparer l'avant et l'après d'un partage).

use bitwarden_vault::CipherView;
use serde::Serialize;
use serde_json::Value;

use crate::item::{Item, Login, LoginUri, PasswordHistoryEntry};
use crate::secret::Secret;

/// Texte ISO 8601 d'une valeur qui se sérialise en date.
///
/// Paramètres : la date.
/// Retour : son texte, vide si la sérialisation échoue.
fn iso<T: Serialize>(date: &T) -> String {
    match serde_json::to_value(date) {
        Ok(Value::String(text)) => text,
        _ => String::new(),
    }
}

/// Section opaque en JSON.
///
/// Paramètres : la section éventuelle.
/// Retour : sa forme JSON, `None` si absente.
fn section<T: Serialize>(value: Option<&T>) -> Option<Value> {
    value.and_then(|inner| serde_json::to_value(inner).ok())
}

/// Convertit la connexion d'une fiche.
///
/// Paramètres : la fiche déchiffrée.
/// Retour : la connexion, `None` pour une fiche sans connexion.
fn login(view: &CipherView) -> Option<Login> {
    view.login.as_ref().map(|inner| Login {
        username: inner.username.clone(),
        password: inner.password.clone().map(Secret::new),
        totp: inner.totp.clone().map(Secret::new),
        uris: inner
            .uris
            .iter()
            .flatten()
            .filter_map(|entry| entry.uri.clone())
            .map(|uri| LoginUri { uri })
            .collect(),
    })
}

/// Convertit une fiche déchiffrée du SDK.
///
/// Paramètres : la fiche.
/// Retour : l'`Item` du moteur.
#[must_use]
pub fn item_from_view(view: &CipherView) -> Item {
    Item {
        id: view.id.map_or_else(String::new, |id| id.to_string()),
        name: view.name.clone(),
        kind: serde_json::to_value(view.r#type)
            .ok()
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        folder_id: view.folder_id.map(|id| id.to_string()),
        revision_date: iso(&view.revision_date),
        organization_id: view.organization_id.map(|id| id.to_string()),
        collection_ids: view.collection_ids.iter().map(ToString::to_string).collect(),
        deleted_date: view.deleted_date.as_ref().map(iso),
        archived_date: view.archived_date.as_ref().map(iso),
        notes: view.notes.clone().map(Secret::new),
        login: login(view),
        has_attachments: view.attachments.as_ref().is_some_and(|list| !list.is_empty()),
        password_history: view
            .password_history
            .iter()
            .flatten()
            .map(|entry| PasswordHistoryEntry {
                password: Secret::new(entry.password.clone()),
                last_used_date: Some(iso(&entry.last_used_date)),
            })
            .collect(),
        secure_note: section(view.secure_note.as_ref()),
        card: section(view.card.as_ref()),
        identity: section(view.identity.as_ref()),
        ssh_key: section(view.ssh_key.as_ref()),
        fields: section(view.fields.as_ref()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {

    use super::*;

    /// Fiche de connexion factice du SDK.
    pub(crate) fn sample_view() -> CipherView {
        let json = serde_json::json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "organizationId": null, "folderId": "22222222-2222-4222-8222-222222222222",
            "collectionIds": [], "key": null, "name": "Fiche factice", "notes": "note-factice",
            "type": 1, "login": {"username": "alice", "password": "factice-mdp", "passwordRevisionDate": null,
                "uris": [{"uri": "https://example.invalid", "match": null, "uriChecksum": null}],
                "totp": "JBSWY3DPEHPK3PXP", "autofillOnPageLoad": null, "httpAuthenticationEnabled": null,
                "fido2Credentials": null},
            "identity": null, "card": null, "secureNote": null, "sshKey": null, "bankAccount": null,
            "driversLicense": null, "passport": null, "favorite": false, "reprompt": 0,
            "organizationUseTotp": false, "edit": true, "permissions": null, "viewPassword": true,
            "localData": null, "attachments": null, "fields": null,
            "passwordHistory": [{"password": "ancien-mdp", "lastUsedDate": "2026-01-02T03:04:05Z"}],
            "creationDate": "2026-01-01T00:00:00Z", "deletedDate": null,
            "revisionDate": "2026-11-16T10:00:00.123456Z", "archivedDate": null
        });
        serde_json::from_value(json).unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn converts_a_login_view() {
        let item = item_from_view(&sample_view());
        assert_eq!((item.kind, item.name.as_str()), (1, "Fiche factice"));
        assert_eq!(item.revision_date, "2026-11-16T10:00:00.123456Z");
        assert_eq!(item.folder_id.as_deref(), Some("22222222-2222-4222-8222-222222222222"));
        let login = item.login.unwrap_or_default();
        assert_eq!(login.username.as_deref(), Some("alice"));
        assert_eq!(login.uris[0].uri, "https://example.invalid");
        assert_eq!(login.totp.as_ref().map(Secret::expose), Some("JBSWY3DPEHPK3PXP"));
        assert_eq!(item.password_history[0].password.expose(), "ancien-mdp");
        assert!(item.deleted_date.is_none() && !item.has_attachments);
    }

    #[test]
    fn debug_of_a_converted_item_hides_secrets() {
        let text = format!("{:?}", item_from_view(&sample_view()));
        assert!(!text.contains("factice-mdp") && !text.contains("note-factice") && !text.contains("JBSWY3"));
    }
}
