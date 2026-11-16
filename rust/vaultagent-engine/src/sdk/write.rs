//! Écritures de fiches : création, mise à jour avec contrôle de révision, corbeille et restauration.

use bitwarden_collections::collection::CollectionId;
use bitwarden_core::OrganizationId;
use bitwarden_vault::{
    CipherCreateRequest, CipherRepromptType, CipherViewType, FolderId, LoginUriView, LoginView, SecureNoteType,
    SecureNoteView, VaultClientExt as _,
};
use serde_json::Value;
use vaultagent_core::{ItemKind, NewItem, NewLogin, Result, RevisionDate, Uuid};

use crate::engine::stage_error;
use crate::sdk::patch::edit_request;
use crate::sdk::read::{cipher_id, read_error};
use crate::sdk::session::Session;
use crate::sdk::write_errors::{create_error, delete_error, edit_error, restore_error};

/// Convertit un identifiant validé en identifiant du SDK.
///
/// Paramètres : l'identifiant.
/// Retour : l'identifiant du SDK.
///
/// # Errors
///
/// `write:rejected` si la forme n'est pas celle d'un identifiant du SDK.
pub(super) fn sdk_id<T: std::str::FromStr>(id: &Uuid) -> Result<T> {
    id.as_str().parse().map_err(|_| stage_error("write", "rejected"))
}

/// Connexion du SDK pour une connexion à créer.
///
/// Paramètres : la connexion validée.
/// Retour : la vue du SDK, adresses sans somme de contrôle.
fn login_view(login: &NewLogin) -> LoginView {
    LoginView {
        username: login.username.clone(),
        password: login.password.clone(),
        password_revision_date: None,
        uris: (!login.uris.is_empty()).then(|| {
            login
                .uris
                .iter()
                .map(|address| LoginUriView {
                    uri: Some(address.as_str().to_owned()),
                    r#match: None,
                    uri_checksum: None,
                })
                .collect()
        }),
        totp: login.totp.clone(),
        autofill_on_page_load: None,
        http_authentication_enabled: None,
        fido2_credentials: None,
    }
}

/// Requête de création du SDK pour une fiche validée.
///
/// Paramètres : la fiche.
/// Retour : la requête.
///
/// # Errors
///
/// `write:rejected` si un identifiant n'a pas la forme attendue.
fn create_request(item: &NewItem) -> Result<CipherCreateRequest> {
    let data = match (&item.kind, &item.login) {
        (ItemKind::Login, login) => CipherViewType::Login(login_view(&login.clone().unwrap_or_default())),
        (ItemKind::SecureNote, _) => CipherViewType::SecureNote(SecureNoteView {
            r#type: SecureNoteType::Generic,
        }),
    };
    let organization_id: Option<OrganizationId> = item.organization_id.as_ref().map(sdk_id).transpose()?;
    let folder_id: Option<FolderId> = item.folder_id.as_ref().map(sdk_id).transpose()?;
    let collection_ids: Vec<CollectionId> = item.collection_ids.iter().map(sdk_id).collect::<Result<_>>()?;
    Ok(CipherCreateRequest {
        organization_id,
        collection_ids,
        folder_id,
        name: item.name.as_str().to_owned(),
        notes: item.notes.clone(),
        favorite: false,
        reprompt: CipherRepromptType::None,
        r#type: data,
        fields: Vec::new(),
        archived_date: None,
    })
}

/// Crée une fiche.
///
/// Paramètres : la session et la fiche validée.
/// Retour : l'identifiant de la fiche créée.
///
/// # Errors
///
/// `write:*`.
pub async fn create_item(session: &Session, item: &NewItem) -> Result<String> {
    let created = session
        .client
        .vault()
        .ciphers()
        .create(create_request(item)?)
        .await
        .map_err(|e| create_error("write", &e))?;
    created
        .id
        .map(|id| id.to_string())
        .ok_or_else(|| stage_error("write", "rejected"))
}

/// Met à jour une fiche : resynchronise, relit, contrôle la révision, applique le patch puis écrit.
///
/// Paramètres : la session, la fiche, la révision attendue, le patch et l'organisation de la fiche.
///
/// # Errors
///
/// `items:not-found` si la fiche manque, `RevisionConflict` si sa révision a changé (ou si le serveur la juge
/// périmée), `write:rejected` sans droit d'édition ou si la politique interdit les fiches personnelles,
/// `InvalidPatch` et `ItemRefused` du patch, `sync:*` et `write:*`.
pub async fn update_item(
    session: &Session,
    id: &Uuid,
    expected_revision: &RevisionDate,
    patch: &Value,
    organization_id: Option<&Uuid>,
) -> Result<()> {
    session.resync().await?;
    let view = session
        .client
        .vault()
        .ciphers()
        .get(cipher_id(id)?)
        .await
        .map_err(|e| read_error(&e))?;
    let request = edit_request(
        &view,
        expected_revision,
        patch,
        organization_id,
        session.orgs.data_ownership(),
    )?;
    let ciphers = session.client.vault().ciphers();
    Box::pin(ciphers.edit(request))
        .await
        .map(|_| ())
        .map_err(|e| edit_error(&e))
}

/// Met une fiche à la corbeille.
///
/// Paramètres : la session et l'identifiant.
///
/// # Errors
///
/// `write:*`.
pub async fn trash_item(session: &Session, id: &Uuid) -> Result<()> {
    let target = sdk_id(id)?;
    session
        .client
        .vault()
        .ciphers()
        .soft_delete(target)
        .await
        .map_err(|e| delete_error(&e))
}

/// Restaure une fiche de la corbeille.
///
/// Paramètres : la session et l'identifiant.
///
/// # Errors
///
/// `write:*`.
pub async fn restore_item(session: &Session, id: &Uuid) -> Result<()> {
    let target = sdk_id(id)?;
    session
        .client
        .vault()
        .ciphers()
        .restore(target)
        .await
        .map(|_| ())
        .map_err(|e| restore_error(&e))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    const ORG: &str = "22222222-2222-4222-8222-222222222222";
    const COLLECTION: &str = "33333333-3333-4333-8333-333333333333";

    #[test]
    fn a_login_is_converted_with_its_addresses_and_context() {
        let item = NewItem::parse(&json!({
            "type": 1, "name": "Connexion", "folderId": "44444444-4444-4444-8444-444444444444",
            "notes": "n", "organizationId": ORG, "collectionIds": [COLLECTION],
            "login": {"username": "alice", "password": "factice", "uris": [{"uri": "https://a.example.invalid"}]}
        }))
        .unwrap();
        let request = create_request(&item).unwrap();
        assert_eq!(request.name, "Connexion");
        assert!(request.organization_id.is_some() && request.folder_id.is_some());
        assert_eq!(request.collection_ids.len(), 1);
        let CipherViewType::Login(login) = &request.r#type else {
            unreachable!()
        };
        assert_eq!(login.username.as_deref(), Some("alice"));
        assert_eq!(login.uris.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn a_note_is_converted_without_organization() {
        let item = NewItem::parse(&json!({"type": 2, "name": "Note", "notes": "texte"})).unwrap();
        let request = create_request(&item).unwrap();
        assert!(matches!(request.r#type, CipherViewType::SecureNote(_)));
        assert!(request.organization_id.is_none() && request.collection_ids.is_empty());
    }
}
