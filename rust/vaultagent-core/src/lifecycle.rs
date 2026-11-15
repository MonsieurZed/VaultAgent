//! Cycle de vie des fiches et dossiers (`delete`, `restore`, `update-folder`, `delete-folder`) : contrôles d'avant
//! écriture et confirmation par relecture, sans moteur.

use crate::error::{ErrorCode, Result};
use crate::jsval::field_truthy;
use crate::ledger::Receipt;
use crate::projection::visible;
use crate::request::{WriteAction, WriteRequest};
use crate::revision::RevisionDate;
use serde_json::Value;

/// Droit à contrôler auprès du moteur quand la fiche appartient à une organisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharingScope {
    /// Organisation propriétaire de la fiche.
    pub organization_id: String,
    /// Collections de la fiche.
    pub collection_ids: Vec<String>,
}

/// Ce que la relecture a rendu : une fiche, ou la liste des dossiers.
#[derive(Debug, Clone, Copy)]
pub enum Observed<'a> {
    /// Fiche relue par `GET /object/item/<id>`.
    Item(&'a Value),
    /// Dossiers listés par `GET /list/object/folders`.
    Folders(&'a [Value]),
}

/// Contrôle la fiche ciblée avant corbeille ou restauration : état, révision, propriétaire.
///
/// Paramètres : la requête (`delete` ou `restore`), la fiche lue dans le coffre.
/// Retour : le droit de partage à contrôler si la fiche est commune, sinon `None`.
/// # Errors
///
/// `ItemRefused` (fiche archivée, d'un autre type, déjà en corbeille pour `delete`, hors corbeille pour `restore`,
/// ou d'un autre propriétaire), `RevisionConflict` (dates normalisées différentes), `RequestRefused` (date de la
/// fiche illisible), `InvalidWrite` (action qui n'est pas un cycle de vie de fiche).
pub fn check_item_lifecycle(request: &WriteRequest, item: &Value) -> Result<Option<SharingScope>> {
    let (restoring, expected) = match request.action() {
        WriteAction::Delete { expected_revision, .. } => (false, expected_revision),
        WriteAction::Restore { expected_revision, .. } => (true, expected_revision),
        _ => return Err(ErrorCode::InvalidWrite),
    };
    let object = visible(item)?.as_object().ok_or(ErrorCode::ItemRefused)?;
    if field_truthy(object, "deletedDate") != restoring {
        return Err(ErrorCode::ItemRefused);
    }
    let current = object
        .get("revisionDate")
        .and_then(Value::as_str)
        .and_then(RevisionDate::parse)
        .ok_or(ErrorCode::RequestRefused)?;
    if !current.same_instant(expected) {
        return Err(ErrorCode::RevisionConflict);
    }
    let owner = match object.get("organizationId") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.as_str()),
        Some(_) => return Err(ErrorCode::ItemRefused),
    };
    if owner != request.organization_id().map(crate::uuid::Uuid::as_str) {
        return Err(ErrorCode::ItemRefused);
    }
    Ok(owner.map(|organization_id| SharingScope {
        organization_id: organization_id.to_owned(),
        collection_ids: strings(object.get("collectionIds")),
    }))
}

/// Liste de chaînes d'un champ (absent ou autre type : liste vide).
fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.as_str().map(str::to_owned))
        .collect()
}

/// Indique si un dossier de la liste porte cet identifiant.
fn has_folder(folders: &[Value], id: &str) -> bool {
    folders
        .iter()
        .any(|folder| folder.get("id").and_then(Value::as_str) == Some(id))
}

/// Contrôle le dossier ciblé : il existe et, pour un renommage, aucun autre dossier ne porte ce nom.
///
/// Paramètres : la requête (`update-folder` ou `delete-folder`), les dossiers listés.
/// Retour : rien si le contrôle passe.
/// # Errors
///
/// `FolderMissing`, `DuplicateFolder`, `InvalidWrite` (action qui n'est pas un cycle de vie de dossier).
pub fn check_folder(request: &WriteRequest, folders: &[Value]) -> Result<()> {
    let (folder_id, new_name) = match request.action() {
        WriteAction::UpdateFolder { folder_id, name } => (folder_id, Some(name.as_str())),
        WriteAction::DeleteFolder { folder_id } => (folder_id, None),
        _ => return Err(ErrorCode::InvalidWrite),
    };
    if !has_folder(folders, folder_id.as_str()) {
        return Err(ErrorCode::FolderMissing);
    }
    let taken = new_name.is_some_and(|wanted| {
        folders.iter().any(|folder| {
            folder.get("id").and_then(Value::as_str) != Some(folder_id.as_str())
                && folder.get("name").and_then(Value::as_str) == Some(wanted)
        })
    });
    if taken { Err(ErrorCode::DuplicateFolder) } else { Ok(()) }
}

/// Confirme l'effet d'une écriture de cycle de vie à partir de la relecture.
///
/// Paramètres : la requête, la fiche relue (`delete`, `restore`) ou les dossiers listés (`update-folder`,
/// `delete-folder`).
/// Retour : le reçu vérifié si l'effet est constaté, `None` sinon (nouvel essai).
/// # Errors
///
/// `ItemRefused` si la fiche relue est inutilisable, `InvalidWrite` si la relecture ne correspond pas à l'action.
pub fn confirm_lifecycle(request: &WriteRequest, observed: Observed<'_>) -> Result<Option<Receipt>> {
    let operation = request.operation_id().as_str();
    let name = request.action().name();
    match (request.action(), observed) {
        (WriteAction::Delete { item_id, .. } | WriteAction::Restore { item_id, .. }, Observed::Item(saved)) => {
            let object = visible(saved)?.as_object().ok_or(ErrorCode::ItemRefused)?;
            let trashed = field_truthy(object, "deletedDate");
            let revision = object.get("revisionDate").and_then(Value::as_str);
            let done = trashed == matches!(request.action(), WriteAction::Delete { .. });
            Ok(done.then(|| Receipt::verified(item_id.as_str(), operation, name, revision)))
        }
        (
            WriteAction::UpdateFolder {
                folder_id,
                name: wanted,
            },
            Observed::Folders(folders),
        ) => {
            let renamed = folders.iter().any(|folder| {
                folder.get("id").and_then(Value::as_str) == Some(folder_id.as_str())
                    && folder.get("name").and_then(Value::as_str) == Some(wanted.as_str())
            });
            Ok(renamed.then(|| Receipt::verified(folder_id.as_str(), operation, name, None)))
        }
        (WriteAction::DeleteFolder { folder_id }, Observed::Folders(folders)) => {
            let gone = !has_folder(folders, folder_id.as_str());
            Ok(gone.then(|| Receipt::verified(folder_id.as_str(), operation, name, None)))
        }
        _ => Err(ErrorCode::InvalidWrite),
    }
}
