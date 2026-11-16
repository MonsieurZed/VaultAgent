//! Route `share` : aperçu, transfert d'une fiche personnelle vers une organisation, création d'une collection.

use serde_json::Value;
use vaultagent_core::share_checks::{check_confirmation, check_item, check_new_collection, find_organization, preview};
use vaultagent_core::share_verify::{collection_created, item_moved, receipt};
use vaultagent_core::{ErrorCode, Receipt, Registry, ShareAction, ShareRequest, Uuid};

use crate::agent_write::without_vault_id;
use crate::context::Context;
use crate::error::Result;
use crate::guard::Guard;
use crate::http::Response;
use crate::item_value::values_of;
use crate::vaults::VaultContext;
use crate::verify_loop::until_confirmed;
use crate::write_ledger::Operation;

/// `share` : aperçu sans écriture, ou transfert et création avec reçu idempotent.
///
/// Paramètres : le contexte, le coffre, la garde, le corps.
/// Retour : l'aperçu ou le reçu (rejoué éventuellement).
///
/// # Errors
///
/// `invalid-share`, `organization-refused`, `item-refused`, `revision-conflict`, `duplicate-collection`,
/// `operation-conflict`, `receipt-capacity`, `share-indeterminate` après l'intention, `locked`, `unauthorized`.
pub fn share(context: &Context, vault: &VaultContext, guard: &Guard<'_>, body: &Value) -> Result<Response> {
    let input = without_vault_id(body);
    let request = ShareRequest::parse(&input)?;
    guard.authorize()?;
    guard.slot()?;
    let mut operation = None;
    if request.action() != ShareAction::Preview {
        let id = request.operation_id().ok_or(ErrorCode::InvalidShare)?;
        let profile = vault.store.account().ok_or(ErrorCode::NotConfigured)?.profile;
        let (opened, replay) = Operation::open(&vault.store, Registry::Share, &profile, id, request.raw())?;
        if let Some(found) = replay {
            guard.check()?;
            return Ok(Response::json(200, &found));
        }
        operation = Some(opened);
    }
    guard.sync()?;
    let organizations = values_of(&guard.call(|engine| engine.organizations())?)?;
    let organization = find_organization(&organizations, &request)?;
    let mut original = None;
    if request.action() != ShareAction::CreateCollection {
        guard.call(|engine| engine.check_sharing(request.organization_id(), request.collection_ids()))?;
        let item_id = request.item_id().ok_or(ErrorCode::InvalidShare)?;
        let item = guard.item_value(item_id)?;
        check_item(&item)?;
        if request.action() == ShareAction::Preview {
            guard.check()?;
            guard.touch();
            return Ok(Response::json(200, &preview(&request, &item, organization)));
        }
        check_confirmation(&request, &item)?;
        original = Some(item);
    }
    let mut operation = operation.ok_or(ErrorCode::InvalidShare)?;
    operation.ensure_capacity()?;
    if request.action() == ShareAction::CreateCollection {
        check_new_collection(organization, &request)?;
    }
    guard.check()?;
    operation.intent()?;
    let saved = apply(context, guard, &request, original.as_ref(), &mut operation)
        .map_err(|_| ErrorCode::ShareIndeterminate)?;
    guard.touch();
    Ok(Response::json(200, &saved))
}

/// Écrit, relit jusqu'à constater l'effet, puis enregistre le reçu.
fn apply(
    context: &Context,
    guard: &Guard<'_>,
    request: &ShareRequest,
    original: Option<&Value>,
    operation: &mut Operation<'_>,
) -> Result<Receipt> {
    guard.check()?;
    let created = request.action() == ShareAction::CreateCollection;
    let id = if created {
        let name = request.name().unwrap_or_default();
        guard.call(|engine| engine.create_collection(request.organization_id(), name))?
    } else {
        let item_id = request.item_id().ok_or(ErrorCode::InvalidShare)?;
        guard.call(|engine| engine.share_item(item_id, request.organization_id(), request.collection_ids()))?;
        item_id.as_str().to_owned()
    };
    let id = Uuid::parse(&id).ok_or(ErrorCode::RequestRefused)?;
    until_confirmed(context, guard, || {
        guard.sync()?;
        if created {
            let organizations = values_of(&guard.call(|engine| engine.organizations())?)?;
            Ok(collection_created(&organizations, request, &id).then_some(()))
        } else {
            let saved = guard.item_value(&id)?;
            let before = original.unwrap_or(&Value::Null);
            Ok(item_moved(&saved, before, request).then_some(()))
        }
    })?;
    guard.check()?;
    operation.receipt(receipt(request, &id)?)
}
