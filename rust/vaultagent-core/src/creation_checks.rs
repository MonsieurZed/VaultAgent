//! Contrôles d'avant écriture et vérification des créations (`create`, `create-folder`) : doublons, dossier cité,
//! effet constaté par relecture. Sans moteur.

use crate::error::{ErrorCode, Result};
use crate::jsval::field_truthy;
use crate::ledger::Receipt;
use crate::request::{WriteAction, WriteRequest};
use serde_json::Value;

/// Valeur d'un champ au sens de `valeur ?? null`.
fn or_null(value: Option<&Value>) -> Value {
    value.cloned().unwrap_or(Value::Null)
}

/// Refuse une création dont une fiche non supprimée porte déjà le même nom, dossier et organisation.
///
/// Paramètres : la requête `create`, les fiches listées (`GET /list/object/items`).
/// Retour : rien si aucune fiche ne fait doublon.
/// # Errors
///
/// `DuplicateItem` ; `InvalidWrite` si la requête n'est pas un `create`.
pub fn check_create_duplicate(request: &WriteRequest, existing: &[Value]) -> Result<()> {
    if !matches!(request.action(), WriteAction::Create { .. }) {
        return Err(ErrorCode::InvalidWrite);
    }
    let wanted = request.raw().get("item").ok_or(ErrorCode::InvalidWrite)?;
    let duplicate = existing.iter().filter_map(Value::as_object).any(|item| {
        !field_truthy(item, "deletedDate")
            && item.get("name") == wanted.get("name")
            && or_null(item.get("folderId")) == or_null(wanted.get("folderId"))
            && or_null(item.get("organizationId")) == or_null(wanted.get("organizationId"))
    });
    if duplicate {
        Err(ErrorCode::DuplicateItem)
    } else {
        Ok(())
    }
}

/// Cherche le dossier déjà créé sous ce nom pour une requête `create-folder` (reprise après panne).
///
/// Paramètres : la requête `create-folder`, les dossiers listés.
/// Retour : l'identifiant du dossier existant, ou `None` s'il faut le créer.
/// # Errors
///
/// `DuplicateFolder` si plusieurs dossiers portent ce nom ; `InvalidWrite` si la requête n'est pas un
/// `create-folder`.
pub fn check_create_folder(request: &WriteRequest, folders: &[Value]) -> Result<Option<String>> {
    let WriteAction::CreateFolder { name } = request.action() else {
        return Err(ErrorCode::InvalidWrite);
    };
    let mut same = folders
        .iter()
        .filter(|folder| folder.get("name").and_then(Value::as_str) == Some(name.as_str()));
    let first = same.next();
    if same.next().is_some() {
        return Err(ErrorCode::DuplicateFolder);
    }
    Ok(first
        .and_then(|folder| folder.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned))
}

/// Refuse un `folderId` qui ne désigne aucun dossier existant.
///
/// Paramètres : le `folderId` cité (`None` ou `null` : aucun contrôle), les dossiers listés.
/// Retour : rien si le dossier existe ou n'est pas cité.
/// # Errors
///
/// `FolderMissing`.
pub fn check_folder_reference(folder_id: Option<&Value>, folders: &[Value]) -> Result<()> {
    let Some(cited) = folder_id.filter(|value| !value.is_null()) else {
        return Ok(());
    };
    if folders.iter().any(|folder| folder.get("id") == Some(cited)) {
        Ok(())
    } else {
        Err(ErrorCode::FolderMissing)
    }
}

/// Confirme qu'un dossier créé apparaît dans la liste avec son nom.
///
/// Paramètres : la requête `create-folder`, l'identifiant du dossier, les dossiers listés.
/// Retour : le reçu vérifié, ou `None` si le dossier n'est pas encore visible (nouvel essai).
/// # Errors
///
/// `InvalidWrite` si la requête n'est pas un `create-folder`.
pub fn confirm_folder_created(request: &WriteRequest, id: &str, folders: &[Value]) -> Result<Option<Receipt>> {
    let WriteAction::CreateFolder { name } = request.action() else {
        return Err(ErrorCode::InvalidWrite);
    };
    let visible = folders.iter().any(|folder| {
        folder.get("id").and_then(Value::as_str) == Some(id)
            && folder.get("name").and_then(Value::as_str) == Some(name.as_str())
    });
    let operation = request.operation_id().as_str();
    Ok(visible.then(|| Receipt::verified(id, operation, request.action().name(), None)))
}
