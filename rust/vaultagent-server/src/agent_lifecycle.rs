//! Cycle de vie de `write` : corbeille, restauration, renommage et suppression de dossier, avec reçu idempotent.

use vaultagent_core::lifecycle::{Observed, check_folder, check_item_lifecycle, confirm_lifecycle};
use vaultagent_core::{ErrorCode, Receipt, Uuid, WriteAction, WriteRequest};

use crate::context::Context;
use crate::error::Result;
use crate::guard::Guard;
use crate::item_value::values_of;
use crate::verify_loop::until_confirmed;
use crate::write_ledger::Operation;

/// Contrôles d'avant écriture : fiche (état, révision, droit de partage) ou dossier (existence, doublon de nom).
fn check(guard: &Guard<'_>, request: &WriteRequest) -> Result<()> {
    match request.action() {
        WriteAction::Delete { item_id, .. } | WriteAction::Restore { item_id, .. } => {
            let item = guard.item_value(item_id)?;
            if let Some(scope) = check_item_lifecycle(request, &item)? {
                let organization = Uuid::parse(&scope.organization_id).ok_or(ErrorCode::ItemRefused)?;
                let collections: Option<Vec<Uuid>> = scope.collection_ids.iter().map(|id| Uuid::parse(id)).collect();
                let collections = collections.ok_or(ErrorCode::ItemRefused)?;
                guard.call(|engine| engine.check_sharing(&organization, &collections))?;
            }
            Ok(())
        }
        _ => {
            let folders = values_of(&guard.call(|engine| engine.folders())?)?;
            Ok(check_folder(request, &folders)?)
        }
    }
}

/// Exécute l'écriture demandée sur le moteur.
fn perform(guard: &Guard<'_>, request: &WriteRequest) -> Result<()> {
    match request.action() {
        WriteAction::Delete { item_id, .. } => guard.call(|engine| engine.trash_item(item_id)),
        WriteAction::Restore { item_id, .. } => guard.call(|engine| engine.restore_item(item_id)),
        WriteAction::UpdateFolder { folder_id, name } => {
            guard.call(|engine| engine.rename_folder(folder_id, name.as_str()))
        }
        WriteAction::DeleteFolder { folder_id } => guard.call(|engine| engine.delete_folder(folder_id)),
        _ => Err(ErrorCode::InvalidWrite.into()),
    }
}

/// Relit le coffre une fois : reçu partiel si l'effet est constaté.
fn observe(guard: &Guard<'_>, request: &WriteRequest) -> Result<Option<Receipt>> {
    match request.action() {
        WriteAction::Delete { item_id, .. } | WriteAction::Restore { item_id, .. } => {
            let saved = guard.item_value(item_id)?;
            Ok(confirm_lifecycle(request, Observed::Item(&saved))?)
        }
        _ => {
            let folders = values_of(&guard.call(|engine| engine.folders())?)?;
            Ok(confirm_lifecycle(request, Observed::Folders(&folders))?)
        }
    }
}

/// Exécute une opération de cycle de vie, après la synchronisation et le contrôle du registre.
///
/// Paramètres : le contexte, la garde, la requête validée, l'opération ouverte (capacité déjà contrôlée).
/// Retour : le reçu vérifié et enregistré.
///
/// # Errors
///
/// `item-refused`, `revision-conflict`, `folder-missing`, `duplicate-folder` avant l'intention ;
/// `write-indeterminate` après.
pub fn run(
    context: &Context,
    guard: &Guard<'_>,
    request: &WriteRequest,
    operation: &mut Operation<'_>,
) -> Result<Receipt> {
    check(guard, request)?;
    guard.check()?;
    operation.intent()?;
    let outcome = (|| -> Result<Receipt> {
        guard.check()?;
        perform(guard, request)?;
        let receipt = until_confirmed(context, guard, || observe(guard, request))?;
        guard.check()?;
        operation.receipt(receipt)
    })();
    outcome.map_err(|_| ErrorCode::WriteIndeterminate.into())
}
