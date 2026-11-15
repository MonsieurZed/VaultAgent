//! Contrôles purs d'un partage avant écriture : organisation, fiche, confirmation de transfert, doublon de
//! collection, et forme de l'aperçu. Les données viennent du moteur, l'appel se fait ailleurs.

use crate::error::{ErrorCode, Result};
use crate::jsval::{small_int, truthy};
use crate::share_request::ShareRequest;
use crate::uuid::Uuid;
use serde::Serialize;
use serde_json::Value;

/// Aperçu d'un partage : ce qui sera transféré, sans aucun secret.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharePreview {
    /// Fiche concernée.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<Value>,
    /// Nom de la fiche.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Value>,
    /// Date de révision à redonner à l'application.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_revision_date: Option<Value>,
    /// Organisation de destination.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<Value>,
    /// Nom de l'organisation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_name: Option<Value>,
    /// Collections de destination, telles que demandées.
    pub collection_ids: Vec<Uuid>,
    /// Toujours vrai : la fiche quitte le coffre personnel.
    pub transfers_ownership: bool,
    /// Toujours vrai : `apply` exige la confirmation.
    pub requires_confirmation: bool,
}

/// Retrouve l'organisation demandée parmi celles de `sharing-info`.
///
/// Paramètres : les organisations du moteur, la requête.
/// Retour : l'organisation.
/// # Errors
///
/// `organization-refused` si aucune organisation n'a cet identifiant.
pub fn find_organization<'a>(organizations: &'a [Value], request: &ShareRequest) -> Result<&'a Value> {
    organizations
        .iter()
        .find(|org| org.get("id").and_then(Value::as_str) == Some(request.organization_id().as_str()))
        .ok_or(ErrorCode::OrganizationRefused)
}

/// Indique si une valeur a une longueur non nulle (liste ou texte), comme `valeur?.length` en JavaScript.
fn has_length(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Array(list)) => !list.is_empty(),
        Some(Value::String(text)) => !text.is_empty(),
        _ => false,
    }
}

/// Contrôle qu'une fiche est partageable.
///
/// Paramètres : la fiche lue dans le coffre.
/// Retour : rien.
/// # Errors
///
/// `item-refused` si la fiche est absente, déjà dans une organisation, supprimée, archivée, d'un type autre que
/// connexion ou note, ou porte des pièces jointes.
pub fn check_item(item: &Value) -> Result<()> {
    let set = |key: &str| item.get(key).is_some_and(truthy);
    let kind = item.get("type").and_then(small_int);
    let shareable = item.is_object()
        && !set("organizationId")
        && !set("deletedDate")
        && !set("archivedDate")
        && matches!(kind, Some(1 | 2))
        && !has_length(item.get("attachments"));
    if shareable { Ok(()) } else { Err(ErrorCode::ItemRefused) }
}

/// Contrôle la confirmation d'un `apply` : transfert confirmé et date de révision inchangée.
///
/// Paramètres : la requête, la fiche relue.
/// Retour : rien.
/// # Errors
///
/// `revision-conflict` si `confirmOwnershipTransfer` n'est pas exactement `true` ou si la date attendue diffère
/// de celle de la fiche.
pub fn check_confirmation(request: &ShareRequest, item: &Value) -> Result<()> {
    if request.confirmed() && request.expected_revision() == item.get("revisionDate") {
        Ok(())
    } else {
        Err(ErrorCode::RevisionConflict)
    }
}

/// Refuse une collection de même nom (rogné) que celles de l'organisation.
///
/// Paramètres : l'organisation de `sharing-info`, la requête `create-collection`.
/// Retour : rien.
/// # Errors
///
/// `duplicate-collection` si le nom existe déjà.
pub fn check_new_collection(organization: &Value, request: &ShareRequest) -> Result<()> {
    let wanted = request.name().ok_or(ErrorCode::InvalidShare)?;
    let existing = organization.get("collections").and_then(Value::as_array);
    let duplicate = existing
        .into_iter()
        .flatten()
        .any(|collection| collection.get("name").and_then(Value::as_str) == Some(wanted));
    if duplicate {
        Err(ErrorCode::DuplicateCollection)
    } else {
        Ok(())
    }
}

/// Construit l'aperçu d'un partage.
///
/// Paramètres : la requête, la fiche lue, l'organisation.
/// Retour : l'aperçu, avec les valeurs de la fiche et de l'organisation telles que lues.
#[must_use]
pub fn preview(request: &ShareRequest, item: &Value, organization: &Value) -> SharePreview {
    let read = |source: &Value, key: &str| source.get(key).cloned();
    SharePreview {
        item_id: read(item, "id"),
        name: read(item, "name"),
        expected_revision_date: read(item, "revisionDate"),
        organization_id: read(organization, "id"),
        organization_name: read(organization, "name"),
        collection_ids: request.collection_ids().to_vec(),
        transfers_ownership: true,
        requires_confirmation: true,
    }
}
