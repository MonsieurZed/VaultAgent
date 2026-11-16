//! Routes de lecture : `catalog`, `write-preview` et `sharing`. Chacune resynchronise d'abord le coffre.

use serde::Serialize;
use serde_json::Value;
use vaultagent_core::projection::{ItemMetadata, active};
use vaultagent_core::{ErrorCode, ItemDetail, Uuid, catalog_entry, item_detail, item_metadata, visible};
use vaultagent_engine::Organization;

use crate::error::Result;
use crate::guard::Guard;
use crate::http::Response;
use crate::item_value::item_values;

const SEARCH_LIMIT: usize = 200;

/// Dossier de la réponse de `write-preview`.
#[derive(Serialize)]
struct FolderEntry {
    id: String,
    name: String,
}

/// Liste de `write-preview` sans fiche ciblée.
#[derive(Serialize)]
struct Listing {
    folders: Vec<FolderEntry>,
    items: Vec<ItemMetadata>,
    trash: Vec<ItemMetadata>,
}

/// Réponse de `sharing`.
#[derive(Serialize)]
struct Sharing {
    organizations: Vec<Organization>,
}

/// Texte de recherche du corps, au sens de `String(search).slice(0, 200)`.
fn search_text(body: &Value) -> String {
    let text = match body.get("search") {
        None => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) => "null".to_owned(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(_) => "[object Object]".to_owned(),
    };
    text.chars().take(SEARCH_LIMIT).collect()
}

/// `catalog` : présences et origines des connexions, sans aucune valeur.
///
/// Paramètres : la garde, le corps.
/// Retour : la liste des entrées.
///
/// # Errors
///
/// `locked`, `unauthorized` ou l'erreur du moteur.
pub fn catalog(guard: &Guard<'_>, body: &Value) -> Result<Response> {
    guard.authorize()?;
    guard.slot()?;
    guard.sync()?;
    let search = search_text(body);
    let items = guard.call(|engine| engine.items((!search.is_empty()).then_some(search.as_str())))?;
    guard.touch();
    let entries: Vec<_> = item_values(&items)?.iter().filter_map(catalog_entry).collect();
    Ok(Response::json(200, &entries))
}

/// `write-preview` : liste des dossiers, fiches et corbeille, ou détail d'une fiche.
///
/// Paramètres : la garde, le corps (`itemId` facultatif).
/// Retour : la liste ou le détail.
///
/// # Errors
///
/// `invalid-item`, `item-refused`, `locked`, `unauthorized` ou l'erreur du moteur.
pub fn write_preview(guard: &Guard<'_>, body: &Value) -> Result<Response> {
    guard.slot()?;
    guard.sync()?;
    let Some(requested) = body.get("itemId") else {
        return listing(guard);
    };
    let id = requested.as_str().and_then(Uuid::parse).ok_or(ErrorCode::InvalidItem)?;
    let item = guard.item_value(&id)?;
    let detail: ItemDetail = item_detail(visible(&item)?).ok_or(ErrorCode::ItemRefused)?;
    guard.touch();
    Ok(Response::json(200, &detail))
}

/// Liste complète de `write-preview` : dossiers, fiches actives, corbeille.
fn listing(guard: &Guard<'_>) -> Result<Response> {
    let folders = guard
        .call(|engine| engine.folders())?
        .into_iter()
        .map(|folder| FolderEntry {
            id: folder.id,
            name: folder.name,
        })
        .collect();
    let items = guard.call(|engine| engine.items(None))?;
    let trashed = guard.call(|engine| engine.trashed_items())?;
    guard.touch();
    let metadata = |list: Vec<Value>, deleted: bool| -> Vec<ItemMetadata> {
        list.iter()
            .filter(|item| visible(item).is_ok())
            .filter(|item| active(item).is_ok() != deleted)
            .filter_map(item_metadata)
            .collect()
    };
    Ok(Response::json(
        200,
        &Listing {
            folders,
            items: metadata(item_values(&items)?, false),
            trash: metadata(item_values(&trashed)?, true),
        },
    ))
}

/// `sharing` : organisations actives avec leurs collections.
///
/// Paramètres : la garde.
/// Retour : `{organizations}`.
///
/// # Errors
///
/// `locked`, `unauthorized` ou l'erreur du moteur.
pub fn sharing(guard: &Guard<'_>) -> Result<Response> {
    guard.authorize()?;
    guard.slot()?;
    guard.sync()?;
    let organizations = guard.call(|engine| engine.organizations())?;
    guard.touch();
    Ok(Response::json(200, &Sharing { organizations }))
}
