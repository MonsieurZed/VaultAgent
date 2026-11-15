//! Vérification par relecture d'un partage (après `move` ou `create-collection`) et forme du reçu.

use crate::error::{ErrorCode, Result};
use crate::ledger::Receipt;
use crate::share_request::ShareRequest;
use crate::uuid::Uuid;
use crate::verify::same_json;
use serde_json::Value;

/// Champs de la fiche qui doivent rester identiques après le transfert.
pub const COMPARED_FIELDS: [&str; 10] = [
    "name",
    "type",
    "notes",
    "login",
    "secureNote",
    "card",
    "identity",
    "sshKey",
    "fields",
    "passwordHistory",
];

/// Identifiants de collections d'une fiche, triés par unités UTF-16 ; `None` si une entrée n'est pas un texte.
fn sorted_ids(value: Option<&Value>) -> Option<Vec<&str>> {
    let mut ids = match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) => list.iter().map(Value::as_str).collect::<Option<Vec<_>>>()?,
        Some(_) => return None,
    };
    ids.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    Some(ids)
}

/// Indique si la fiche relue après `move` est la fiche d'origine, partagée comme demandé.
///
/// Paramètres : la fiche relue, la fiche d'origine, la requête `apply`.
/// Retour : `true` si l'organisation est la bonne, si les collections (sans doublon, triées) sont celles
/// demandées et si les champs de [`COMPARED_FIELDS`] sont inchangés en forme canonique.
#[must_use]
pub fn item_moved(saved: &Value, original: &Value, request: &ShareRequest) -> bool {
    let organization = saved.get("organizationId").and_then(Value::as_str) == Some(request.organization_id().as_str());
    let mut wanted: Vec<&str> = request.collection_ids().iter().map(Uuid::as_str).collect();
    wanted.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    wanted.dedup();
    let collections = sorted_ids(saved.get("collectionIds")).is_some_and(|saved_ids| saved_ids == wanted);
    let or_null = |item: &Value, key: &str| item.get(key).cloned().unwrap_or(Value::Null);
    organization
        && collections
        && COMPARED_FIELDS
            .iter()
            .all(|key| same_json(&or_null(saved, key), &or_null(original, key)))
}

/// Indique si la collection créée figure, avec son nom, dans l'organisation relue.
///
/// Paramètres : les organisations de `sharing-info`, la requête `create-collection`, l'identifiant rendu par la
/// création.
/// Retour : `true` si l'organisation contient une collection de cet identifiant et du nom demandé.
#[must_use]
pub fn collection_created(organizations: &[Value], request: &ShareRequest, id: &Uuid) -> bool {
    let wanted = request.name();
    organizations
        .iter()
        .filter(|org| org.get("id").and_then(Value::as_str) == Some(request.organization_id().as_str()))
        .filter_map(|org| org.get("collections").and_then(Value::as_array))
        .flatten()
        .any(|collection| {
            collection.get("id").and_then(Value::as_str) == Some(id.as_str())
                && collection.get("name").and_then(Value::as_str) == wanted
        })
}

/// Reçu d'un partage vérifié.
///
/// Paramètres : la requête (`apply` ou `create-collection`), l'identifiant de la fiche ou de la collection.
/// Retour : `{id, operationId, action, organizationId, verified: true}`.
/// # Errors
///
/// `invalid-share` pour un aperçu, qui n'a ni opération ni reçu.
pub fn receipt(request: &ShareRequest, id: &Uuid) -> Result<Receipt> {
    let operation = request.operation_id().ok_or(ErrorCode::InvalidShare)?;
    Ok(Receipt {
        organization_id: Some(request.organization_id().to_string()),
        ..Receipt::verified(id.as_str(), operation.as_str(), request.action().name(), None)
    })
}
