//! Écritures d'organisation : création d'une collection (POST que le SDK ne fait pas, nom chiffré par le SDK) et
//! partage d'une fiche personnelle vers une organisation, suivis d'une resynchronisation.

use bitwarden_collections::collection::{CollectionId, CollectionType, CollectionView};
use bitwarden_core::OrganizationId;
use bitwarden_vault::VaultClientExt as _;
use serde::{Deserialize, Serialize};
use vaultagent_core::{Result, Uuid};

use crate::engine::stage_error;
use crate::sdk::errors::status_category;
use crate::sdk::org;
use crate::sdk::read::{cipher_id, read_error};
use crate::sdk::session::Session;
use crate::sdk::write::sdk_id;
use crate::sdk::write_errors::share_error;

/// Corps du POST de création d'une collection.
#[derive(Serialize)]
struct CollectionBody {
    name: String,
    users: Vec<String>,
    groups: Vec<String>,
}

/// Réponse du serveur à la création d'une collection.
#[derive(Deserialize)]
struct CollectionCreated {
    #[serde(alias = "Id")]
    id: String,
}

/// Collection vide à chiffrer, gérée par l'utilisateur.
///
/// Paramètres : l'organisation et le nom.
/// Retour : la vue à chiffrer.
fn blank_view(organization_id: OrganizationId, name: &str) -> CollectionView {
    CollectionView {
        id: None,
        organization_id,
        name: name.to_owned(),
        external_id: None,
        hide_passwords: false,
        read_only: false,
        manage: true,
        r#type: CollectionType::SharedCollection,
        has_enabled_access_rule: None,
    }
}

/// Crée une collection dans une organisation puis resynchronise.
///
/// Paramètres : la session, l'organisation et le nom en clair.
/// Retour : l'identifiant de la collection créée.
///
/// # Errors
///
/// `sharing:*` si le chiffrement, le POST ou la lecture de la réponse échoue, `sync:*` à la resynchronisation.
pub async fn create_collection(session: &Session, organization_id: &Uuid, name: &str) -> Result<String> {
    let rejected = || stage_error("sharing", "rejected");
    let view = blank_view(organization_id.as_str().parse().map_err(|_| rejected())?, name);
    let encrypted = session
        .client
        .vault()
        .collections()
        .encrypt(view)
        .map_err(|_| rejected())?;
    let body = serde_json::to_string(&CollectionBody {
        name: encrypted.name.to_string(),
        users: Vec::new(),
        groups: Vec::new(),
    })
    .map_err(|_| rejected())?;
    let config = session.client.internal.get_api_configurations();
    let response = config
        .api_config
        .client
        .post(format!(
            "{}/organizations/{}/collections",
            config.api_config.base_path,
            organization_id.as_str()
        ))
        .bearer_auth(session.access_token().await?)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|_| stage_error("sharing", "network"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(stage_error("sharing", status_category(status.as_u16())));
    }
    let created: CollectionCreated = response.json().await.map_err(|_| rejected())?;
    session.resync().await?;
    Ok(created.id)
}

/// Transfère une fiche personnelle à une organisation, dans des collections, puis resynchronise.
///
/// Paramètres : la session, la fiche, l'organisation et les collections visées.
///
/// # Errors
///
/// `sharing:refused` si une collection n'est pas éditable ou si la fiche est déjà à une organisation,
/// `items:not-found`, `sharing:*`, `sync:*`.
pub async fn share_item(session: &Session, id: &Uuid, organization_id: &Uuid, collection_ids: &[Uuid]) -> Result<()> {
    org::check_sharing(session, organization_id, collection_ids)?;
    let view = session
        .client
        .vault()
        .ciphers()
        .get(cipher_id(id)?)
        .await
        .map_err(|e| read_error(&e))?;
    let target: OrganizationId = sdk_id(organization_id).map_err(|_| stage_error("sharing", "refused"))?;
    let collections: Vec<CollectionId> = collection_ids
        .iter()
        .map(sdk_id)
        .collect::<Result<_>>()
        .map_err(|_| stage_error("sharing", "refused"))?;
    let ciphers = session.client.vault().ciphers();
    Box::pin(ciphers.share_cipher(view, target, collections, None))
        .await
        .map_err(|e| share_error(&e))?;
    session.resync().await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_body_has_the_shape_vaultwarden_expects() {
        let body = serde_json::to_value(CollectionBody {
            name: "2.chiffré".into(),
            users: Vec::new(),
            groups: Vec::new(),
        })
        .unwrap();
        assert_eq!(body["name"], "2.chiffré");
        assert!(body["users"].as_array().unwrap().is_empty() && body["groups"].as_array().unwrap().is_empty());
    }

    #[test]
    fn the_created_collection_id_is_read_in_either_case() {
        let lower: CollectionCreated = serde_json::from_str(r#"{"id":"a","object":"collection"}"#).unwrap();
        let upper: CollectionCreated = serde_json::from_str(r#"{"Id":"b"}"#).unwrap();
        assert_eq!((lower.id.as_str(), upper.id.as_str()), ("a", "b"));
    }

    #[test]
    fn a_blank_view_is_manageable_and_unnamed_id() {
        let org = "22222222-2222-4222-8222-222222222222".parse().unwrap();
        let view = blank_view(org, "Nom");
        assert!(view.id.is_none() && view.manage && !view.read_only);
        assert_eq!(view.name, "Nom");
    }
}
