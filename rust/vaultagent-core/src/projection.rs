//! Projections d'une fiche sans secret : entrée de catalogue, métadonnées et détail de `write-preview`.
//! Les noms de champs sont ceux du protocole ; seules des présences (booléens) remplacent les valeurs.

use crate::error::{ErrorCode, Result};
use crate::https::https_origin;
use crate::jsval::{field_truthy, small_int};
use crate::uuid::Uuid;
use serde::Serialize;
use serde_json::{Map, Value};

/// Entrée du catalogue (`POST /api/agent/catalog`) : présences et origines, jamais de valeurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    /// Identifiant de la fiche.
    pub id: String,
    /// Nom de la fiche.
    pub name: String,
    /// Organisation propriétaire.
    pub organization_id: Option<String>,
    /// Collections de la fiche.
    pub collection_ids: Vec<String>,
    /// Origines `https:` distinctes des adresses de la fiche.
    pub origins: Vec<String>,
    /// Un identifiant de connexion existe.
    pub username: bool,
    /// Un mot de passe existe.
    pub password: bool,
    /// Un TOTP existe.
    pub totp: bool,
}

/// Métadonnées d'une fiche (listes `items` et `trash` de `write-preview`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemMetadata {
    /// Identifiant de la fiche.
    pub id: String,
    /// Nom de la fiche.
    pub name: String,
    /// Type de fiche (1 connexion, 2 note).
    #[serde(rename = "type")]
    pub kind: i64,
    /// Dossier, ou `null`.
    pub folder_id: Option<String>,
    /// Date de révision telle que fournie par le moteur.
    pub revision_date: String,
    /// Organisation propriétaire.
    pub organization_id: Option<String>,
    /// Collections de la fiche.
    pub collection_ids: Vec<String>,
}

/// Présence des valeurs d'une fiche.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Presence {
    /// Identifiant de connexion renseigné.
    pub username: bool,
    /// Mot de passe renseigné.
    pub password: bool,
    /// TOTP renseigné.
    pub totp: bool,
    /// Notes renseignées.
    pub notes: bool,
}

/// Détail d'une fiche (`write-preview` avec `itemId`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemDetail {
    /// Métadonnées, aplaties dans l'objet.
    #[serde(flatten)]
    pub metadata: ItemMetadata,
    /// La fiche est dans la corbeille.
    pub deleted: bool,
    /// Adresses enregistrées.
    pub uris: Vec<String>,
    /// Présence des valeurs.
    pub has: Presence,
}

/// Lit une chaîne d'un champ, vide si absente ou d'un autre type.
fn text(map: &Map<String, Value>, key: &str) -> String {
    map.get(key).and_then(Value::as_str).unwrap_or_default().to_owned()
}

/// Lit une chaîne optionnelle ; `null`, absence ou autre type donnent `None`.
fn optional_text(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// Lit une liste de chaînes, vide si absente ; les éléments qui ne sont pas des chaînes sont ignorés.
fn text_list(map: &Map<String, Value>, key: &str) -> Vec<String> {
    let list = map.get(key).and_then(Value::as_array);
    list.map_or_default(|list| list.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
}

/// Adresses (`login.uris[].uri`) d'une fiche.
fn login_uris(item: &Map<String, Value>) -> Vec<String> {
    let entries = item
        .get("login")
        .and_then(|login| login.get("uris"))
        .and_then(Value::as_array);
    entries.map_or_default(|list| {
        list.iter()
            .filter_map(|e| e.get("uri").and_then(Value::as_str).map(str::to_owned))
            .collect()
    })
}

/// Indique si un champ de la connexion est une chaîne non vide.
fn login_has(item: &Map<String, Value>, key: &str) -> bool {
    item.get("login")
        .and_then(|login| login.get(key))
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty())
}

/// Entrée de catalogue d'une fiche.
///
/// Paramètres : la fiche (JSON du moteur).
/// Retour : `None` si ce n'est pas une connexion (type 1) à identifiant textuel.
#[must_use]
pub fn catalog_entry(item: &Value) -> Option<CatalogEntry> {
    let map = item.as_object()?;
    (map.get("type").and_then(small_int) == Some(1)).then_some(())?;
    let mut origins: Vec<String> = Vec::new();
    for origin in login_uris(map).iter().filter_map(|uri| https_origin(uri)) {
        if !origins.contains(&origin) {
            origins.push(origin);
        }
    }
    Some(CatalogEntry {
        id: optional_text(map, "id")?,
        name: text(map, "name"),
        organization_id: optional_text(map, "organizationId"),
        collection_ids: text_list(map, "collectionIds"),
        origins,
        username: login_has(map, "username"),
        password: login_has(map, "password"),
        totp: login_has(map, "totp"),
    })
}

/// Métadonnées d'une fiche.
///
/// Paramètres : la fiche (JSON du moteur).
/// Retour : `None` si l'identifiant ou le type est absent.
#[must_use]
pub fn item_metadata(item: &Value) -> Option<ItemMetadata> {
    let map = item.as_object()?;
    Some(ItemMetadata {
        id: optional_text(map, "id")?,
        name: text(map, "name"),
        kind: map.get("type").and_then(small_int)?,
        folder_id: optional_text(map, "folderId"),
        revision_date: text(map, "revisionDate"),
        organization_id: optional_text(map, "organizationId"),
        collection_ids: text_list(map, "collectionIds"),
    })
}

/// Détail d'une fiche : adresses, état de corbeille et présence des valeurs.
///
/// Paramètres : la fiche (JSON du moteur).
/// Retour : `None` si l'identifiant ou le type est absent.
#[must_use]
pub fn item_detail(item: &Value) -> Option<ItemDetail> {
    let metadata = item_metadata(item)?;
    let map = item.as_object()?;
    Some(ItemDetail {
        metadata,
        deleted: field_truthy(map, "deletedDate"),
        uris: login_uris(map),
        has: Presence {
            username: login_has(map, "username"),
            password: login_has(map, "password"),
            totp: login_has(map, "totp"),
            notes: map
                .get("notes")
                .and_then(Value::as_str)
                .is_some_and(|notes| !notes.is_empty()),
        },
    })
}

/// Refuse une fiche archivée, d'un type non géré ou sans identifiant valide ; accepte la corbeille.
///
/// Paramètres : la fiche lue dans le coffre.
/// Retour : la même fiche.
/// # Errors
///
/// `ItemRefused`.
pub fn visible(item: &Value) -> Result<&Value> {
    let map = item.as_object().ok_or(ErrorCode::ItemRefused)?;
    let valid_id = map.get("id").and_then(Value::as_str).and_then(Uuid::parse).is_some();
    let kind_ok = map
        .get("type")
        .and_then(small_int)
        .is_some_and(|kind| kind == 1 || kind == 2);
    if valid_id && kind_ok && !field_truthy(map, "archivedDate") {
        Ok(item)
    } else {
        Err(ErrorCode::ItemRefused)
    }
}

/// Refuse en plus une fiche placée dans la corbeille.
///
/// Paramètres : la fiche lue dans le coffre.
/// Retour : la même fiche.
/// # Errors
///
/// `ItemRefused`.
pub fn active(item: &Value) -> Result<&Value> {
    let checked = visible(item)?;
    if checked.as_object().is_some_and(|map| field_truthy(map, "deletedDate")) {
        Err(ErrorCode::ItemRefused)
    } else {
        Ok(checked)
    }
}
