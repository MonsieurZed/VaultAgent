//! Dossiers personnels : liste, création, renommage et suppression (le SDK ne supprime pas : appel direct de
//! l'API `DELETE /folders/{id}` par son client, puis resynchronisation pour que les fiches perdent leur dossier).

use bitwarden_vault::{CreateFolderError, EditFolderError, FolderAddEditRequest, FolderId, VaultClientExt as _};
use vaultagent_core::{ErrorCode, Result, Uuid};

use crate::engine::stage_error;
use crate::sdk::errors::api_category;
use crate::sdk::session::Session;
use crate::structure::Folder;

/// Identifiant de dossier du SDK.
///
/// Paramètres : l'identifiant validé.
/// Retour : l'identifiant du SDK.
///
/// # Errors
///
/// `write:rejected` si la forme n'est pas celle d'un UUID du SDK.
fn folder_id(id: &Uuid) -> Result<FolderId> {
    id.as_str().parse().map_err(|_| stage_error("write", "rejected"))
}

/// Code d'une erreur de création de dossier.
///
/// Paramètres : l'erreur du SDK.
/// Retour : `write:<catégorie>`.
fn create_error(error: &CreateFolderError) -> ErrorCode {
    match error {
        CreateFolderError::Api(inner) => stage_error("write", api_category(inner)),
        _ => stage_error("write", "rejected"),
    }
}

/// Code d'une erreur de renommage de dossier.
///
/// Paramètres : l'erreur du SDK.
/// Retour : `write:<catégorie>`.
fn edit_error(error: &EditFolderError) -> ErrorCode {
    match error {
        EditFolderError::Api(inner) => stage_error("write", api_category(inner)),
        _ => stage_error("write", "rejected"),
    }
}

/// Liste les dossiers personnels.
///
/// Paramètres : la session.
/// Retour : les dossiers déchiffrés.
///
/// # Errors
///
/// `folders:rejected` si le dépôt est illisible.
pub async fn folders(session: &Session) -> Result<Vec<Folder>> {
    let views = session
        .client
        .vault()
        .folders()
        .list()
        .await
        .map_err(|_| stage_error("folders", "rejected"))?;
    Ok(views
        .into_iter()
        .filter_map(|view| {
            view.id.map(|id| Folder {
                id: id.to_string(),
                name: view.name,
            })
        })
        .collect())
}

/// Crée un dossier.
///
/// Paramètres : la session et le nom.
/// Retour : l'identifiant du dossier créé.
///
/// # Errors
///
/// `write:*`.
pub async fn create_folder(session: &Session, name: &str) -> Result<String> {
    let created = session
        .client
        .vault()
        .folders()
        .create(FolderAddEditRequest { name: name.to_owned() })
        .await
        .map_err(|e| create_error(&e))?;
    created
        .id
        .map(|id| id.to_string())
        .ok_or_else(|| stage_error("write", "rejected"))
}

/// Renomme un dossier.
///
/// Paramètres : la session, l'identifiant et le nouveau nom.
///
/// # Errors
///
/// `write:*`.
pub async fn rename_folder(session: &Session, id: &Uuid, name: &str) -> Result<()> {
    session
        .client
        .vault()
        .folders()
        .edit(folder_id(id)?, FolderAddEditRequest { name: name.to_owned() })
        .await
        .map(|_| ())
        .map_err(|e| edit_error(&e))
}

/// Supprime un dossier puis resynchronise le coffre.
///
/// Paramètres : la session et l'identifiant.
///
/// # Errors
///
/// `write:*`, ou `sync:*` si la resynchronisation échoue.
pub async fn delete_folder(session: &Session, id: &Uuid) -> Result<()> {
    let target = folder_id(id)?.to_string();
    session
        .client
        .internal
        .get_api_configurations()
        .api_client
        .folders_api()
        .delete(&target)
        .await
        .map_err(|e| stage_error("write", api_category(&e)))?;
    session.resync().await
}
