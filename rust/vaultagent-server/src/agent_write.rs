//! Route `write` : validation, idempotence, contrôles d'avant écriture, écriture puis vérification par relecture.
//! Cette partie traite `create`, `update` et `create-folder` ; le cycle de vie est dans `agent_lifecycle`.

use serde_json::Value;
use vaultagent_core::verify::{matches_created, verify_update};
use vaultagent_core::{
    ErrorCode, Receipt, Registry, Uuid, WriteAction, WriteRequest, active, apply_write_patch,
    creation_checks::{check_create_duplicate, check_create_folder, check_folder_reference, confirm_folder_created},
};

use crate::agent_lifecycle;
use crate::context::Context;
use crate::error::Result;
use crate::guard::Guard;
use crate::http::Response;
use crate::item_value::{item_values, values_of};
use crate::vaults::VaultContext;
use crate::verify_loop::until_confirmed;
use crate::write_ledger::Operation;

/// État relevé avant l'écriture d'une création ou d'une mise à jour.
struct Prepared {
    before: Option<Value>,
    expected: Option<Value>,
    existing_folder: Option<String>,
}

/// Copie du corps sans `vaultId`.
pub(crate) fn without_vault_id(body: &Value) -> Value {
    let mut copy = body.clone();
    if let Value::Object(map) = &mut copy {
        map.remove("vaultId");
    }
    copy
}

/// `write` : crée, modifie, supprime ou restaure une fiche ou un dossier.
///
/// Paramètres : le contexte, le coffre, la garde, le corps.
/// Retour : le reçu vérifié, éventuellement rejoué.
///
/// # Errors
///
/// Codes de validation (`invalid-write`, `invalid-patch`), contrôles d'avant écriture, `operation-conflict`,
/// `receipt-capacity`, `write-indeterminate` après l'intention, `locked`, `unauthorized`.
pub fn write(context: &Context, vault: &VaultContext, guard: &Guard<'_>, body: &Value) -> Result<Response> {
    let input = without_vault_id(body);
    let request = WriteRequest::parse(&input)?;
    guard.authorize()?;
    guard.slot()?;
    let profile = vault.store.account().ok_or(ErrorCode::NotConfigured)?.profile;
    let (mut operation, replay) = Operation::open(
        &vault.store,
        Registry::Write,
        &profile,
        request.operation_id(),
        request.raw(),
    )?;
    if let Some(receipt) = replay {
        guard.check()?;
        return Ok(Response::json(200, &receipt));
    }
    operation.ensure_capacity()?;
    guard.sync()?;
    let receipt = match request.action() {
        WriteAction::Delete { .. }
        | WriteAction::Restore { .. }
        | WriteAction::UpdateFolder { .. }
        | WriteAction::DeleteFolder { .. } => agent_lifecycle::run(context, guard, &request, &mut operation)?,
        _ => {
            let prepared = prepare(guard, &request)?;
            guard.check()?;
            operation.intent()?;
            execute(context, guard, &request, &prepared, &mut operation).map_err(|_| ErrorCode::WriteIndeterminate)?
        }
    };
    guard.touch();
    Ok(Response::json(200, &receipt))
}

/// Contrôles d'avant écriture selon l'action.
fn prepare(guard: &Guard<'_>, request: &WriteRequest) -> Result<Prepared> {
    let mut prepared = Prepared {
        before: None,
        expected: None,
        existing_folder: None,
    };
    let mut folder_id: Option<Value> = None;
    match request.action() {
        WriteAction::Update {
            item_id,
            expected_revision,
            patch,
        } => {
            let item = guard.item_value(item_id)?;
            let mut expected = active(&item)?.clone();
            prepared.before = Some(expected.clone());
            apply_write_patch(&mut expected, expected_revision, patch, request.organization_id())?;
            prepared.expected = Some(expected);
            folder_id = patch.get("folderId").cloned();
        }
        WriteAction::Create { item } => {
            let existing = item_values(&guard.call(|engine| engine.items(None))?)?;
            check_create_duplicate(request, &existing)?;
            if let Some(organization) = &item.organization_id {
                guard.call(|engine| engine.check_sharing(organization, &item.collection_ids))?;
            }
            folder_id = request.raw().get("item").and_then(|item| item.get("folderId")).cloned();
        }
        _ => {
            let folders = values_of(&guard.call(|engine| engine.folders())?)?;
            prepared.existing_folder = check_create_folder(request, &folders)?;
        }
    }
    if let Some(wanted) = folder_id.filter(|value| !value.is_null()) {
        let folders = values_of(&guard.call(|engine| engine.folders())?)?;
        check_folder_reference(Some(&wanted), &folders)?;
    }
    Ok(prepared)
}

/// Écrit puis relit jusqu'à constater l'effet ; enregistre le reçu.
fn execute(
    context: &Context,
    guard: &Guard<'_>,
    request: &WriteRequest,
    prepared: &Prepared,
    operation: &mut Operation<'_>,
) -> Result<Receipt> {
    guard.check()?;
    let receipt = match request.action() {
        WriteAction::CreateFolder { name } => {
            let id = match &prepared.existing_folder {
                Some(id) => id.clone(),
                None => guard.call(|engine| engine.create_folder(name.as_str()))?,
            };
            Uuid::parse(&id).ok_or(ErrorCode::RequestRefused)?;
            until_confirmed(context, guard, || {
                let folders = values_of(&guard.call(|engine| engine.folders())?)?;
                Ok(confirm_folder_created(request, &id, &folders)?)
            })?
        }
        WriteAction::Create { item } => {
            let id = guard.call(|engine| engine.create_item(item))?;
            let uuid = Uuid::parse(&id).ok_or(ErrorCode::RequestRefused)?;
            let wanted = request.raw().get("item").cloned().unwrap_or(Value::Null);
            let saved = until_confirmed(context, guard, || {
                let saved = guard.item_value(&uuid)?;
                Ok(matches_created(active(&saved)?, &wanted).then_some(saved))
            })?;
            receipt_for(request, &id, &saved)
        }
        WriteAction::Update {
            item_id,
            expected_revision,
            patch,
        } => {
            guard.call(|engine| engine.update_item(item_id, expected_revision, patch, request.organization_id()))?;
            let (before, expected) = (prepared.before.clone(), prepared.expected.clone());
            let (before, expected) = (before.unwrap_or(Value::Null), expected.unwrap_or(Value::Null));
            let saved = until_confirmed(context, guard, || {
                let saved = guard.item_value(item_id)?;
                Ok(verify_update(&before, &expected, active(&saved)?, patch).then_some(saved))
            })?;
            receipt_for(request, item_id.as_str(), &saved)
        }
        _ => return Err(ErrorCode::InvalidWrite.into()),
    };
    guard.check()?;
    operation.receipt(receipt)
}

/// Reçu d'une écriture de fiche, avec la date de révision relue.
fn receipt_for(request: &WriteRequest, id: &str, saved: &Value) -> Receipt {
    let revision = saved.get("revisionDate").and_then(Value::as_str);
    Receipt::verified(id, request.operation_id().as_str(), request.action().name(), revision)
}
