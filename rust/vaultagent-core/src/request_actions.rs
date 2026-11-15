//! Contrôles propres à chaque action d'écriture (`validate` et `validateLifecycle` de Node), avant toute requête au moteur.

use crate::error::{ErrorCode, Result};
use crate::jsval::{field_truthy, object, only_keys};
use crate::name::{BoundedName, FOLDER_NAME_MAX};
use crate::new_item::{NewItem, optional_uuid};
use crate::patch::apply_write_patch;
use crate::revision::RevisionDate;
use crate::uuid::Uuid;
use serde_json::{Map, Value};

/// Refuse la requête si l'un des champs nommés est « vrai » au sens de JavaScript.
fn forbid(map: &Map<String, Value>, keys: &[&str]) -> Result<()> {
    if keys.iter().any(|key| field_truthy(map, key)) {
        Err(ErrorCode::InvalidWrite)
    } else {
        Ok(())
    }
}

/// Lit un UUID obligatoire d'un champ.
fn required_uuid(map: &Map<String, Value>, key: &str) -> Result<Uuid> {
    map.get(key)
        .and_then(Value::as_str)
        .and_then(Uuid::parse)
        .ok_or(ErrorCode::InvalidWrite)
}

/// Lit la date de révision attendue, obligatoire.
fn required_revision(map: &Map<String, Value>) -> Result<RevisionDate> {
    map.get("expectedRevisionDate")
        .and_then(Value::as_str)
        .and_then(RevisionDate::parse)
        .ok_or(ErrorCode::InvalidWrite)
}

/// Lit `item: {name}` d'un dossier.
fn folder_name(map: &Map<String, Value>) -> Result<BoundedName> {
    let item = map
        .get("item")
        .and_then(|item| only_keys(item, &["name"]))
        .ok_or(ErrorCode::InvalidWrite)?;
    item.get("name")
        .and_then(Value::as_str)
        .and_then(|text| BoundedName::parse(text, FOLDER_NAME_MAX))
        .ok_or(ErrorCode::InvalidWrite)
}

/// Valide une création de fiche.
///
/// Erreurs : `InvalidWrite` si un champ d'une autre action est présent ou si `item` est invalide.
pub fn create(map: &Map<String, Value>) -> Result<NewItem> {
    forbid(map, &["folderId", "itemId", "patch", "expectedRevisionDate"])?;
    NewItem::parse(map.get("item").unwrap_or(&Value::Null))
}

/// Valide une création de dossier.
///
/// Erreurs : `InvalidWrite` si un champ d'une autre action est présent ou si le nom est invalide.
pub fn create_folder(map: &Map<String, Value>) -> Result<BoundedName> {
    forbid(map, &["folderId", "itemId", "patch", "expectedRevisionDate"])?;
    folder_name(map)
}

/// Valide une mise à jour de fiche, y compris la forme du patch par application à une fiche témoin.
///
/// Retour : l'identifiant de fiche, la révision attendue et le patch.
/// Erreurs : `InvalidWrite` pour la forme de la requête, `InvalidPatch` pour le patch.
pub fn update(map: &Map<String, Value>, organization: Option<&Uuid>) -> Result<(Uuid, RevisionDate, Value)> {
    forbid(map, &["folderId", "item"])?;
    let item_id = required_uuid(map, "itemId")?;
    let revision = required_revision(map)?;
    let patch = map.get("patch").cloned().unwrap_or(Value::Null);
    let removable: Vec<Value> = patch
        .get("removeUris")
        .and_then(Value::as_array)
        .map_or_default(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(|uri| object(vec![("uri", Value::from(uri))]))
                .collect()
        });
    let owner = organization.map(Uuid::as_str);
    let mut witness = object(vec![
        ("type", Value::from(1)),
        ("login", object(vec![("uris", Value::Array(removable))])),
        ("revisionDate", Value::from(revision.as_str())),
        ("organizationId", owner.map_or(Value::Null, Value::from)),
    ]);
    apply_write_patch(&mut witness, &revision, &patch, organization)?;
    Ok((item_id, revision, patch))
}

/// Contrôles communs aux actions de cycle de vie : ni `patch`, ni `item` hors renommage de dossier.
fn lifecycle_common(map: &Map<String, Value>, rename: bool) -> Result<()> {
    if field_truthy(map, "patch") || !rename && field_truthy(map, "item") {
        Err(ErrorCode::InvalidWrite)
    } else {
        Ok(())
    }
}

/// Valide une mise à la corbeille ou une restauration de fiche.
///
/// Retour : l'identifiant de fiche et la révision attendue.
/// Erreurs : `InvalidWrite` si la forme est invalide.
pub fn item_lifecycle(map: &Map<String, Value>) -> Result<(Uuid, RevisionDate)> {
    lifecycle_common(map, false)?;
    forbid(map, &["folderId"])?;
    Ok((required_uuid(map, "itemId")?, required_revision(map)?))
}

/// Valide un renommage ou une suppression de dossier.
///
/// Paramètres : la requête et `Some(nom)` attendu seulement pour un renommage.
/// Retour : l'identifiant de dossier et, pour un renommage, le nouveau nom.
/// Erreurs : `InvalidWrite` si la forme est invalide.
pub fn folder_lifecycle(map: &Map<String, Value>, rename: bool) -> Result<(Uuid, Option<BoundedName>)> {
    lifecycle_common(map, rename)?;
    let folder_id = required_uuid(map, "folderId")?;
    forbid(map, &["itemId", "expectedRevisionDate", "organizationId"])?;
    Ok((folder_id, if rename { Some(folder_name(map)?) } else { None }))
}

/// Lit `organizationId` de premier niveau : absent ou `null` est permis, sinon un UUID.
pub fn organization(map: &Map<String, Value>) -> Result<Option<Uuid>> {
    optional_uuid(map, "organizationId")
}
