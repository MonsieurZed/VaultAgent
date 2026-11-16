//! Authentification d'un client agent : coffres où son jeton est valable et choix du coffre de la requête.

use std::sync::Arc;

use serde_json::Value;
use vaultagent_core::vault_select::{VaultDevices, authorized, select};
use vaultagent_core::{DeviceRecord, ErrorCode};

use crate::context::Context;
use crate::error::Result;
use crate::vaults::VaultContext;

/// Instantané des appareils d'un coffre.
struct Snapshot {
    id: String,
    profile: Option<String>,
    records: Vec<DeviceRecord>,
}

/// Prend un instantané des appareils de chaque coffre.
fn snapshots(context: &Context) -> Vec<Snapshot> {
    context
        .vaults
        .list()
        .iter()
        .map(|vault| Snapshot {
            id: vault.id.clone(),
            profile: vault.store.account().map(|account| account.profile),
            records: vault.store.devices(),
        })
        .collect()
}

/// Vues attendues par `vaultagent-core` sur les instantanés.
fn views(snapshots: &[Snapshot]) -> Vec<VaultDevices<'_>> {
    snapshots
        .iter()
        .map(|snapshot| VaultDevices {
            id: &snapshot.id,
            profile: snapshot.profile.as_deref(),
            records: &snapshot.records,
        })
        .collect()
}

/// Coffres dont le jeton authentifie un appareil.
///
/// Paramètres : le contexte, l'en-tête `authorization`.
/// Retour : les coffres autorisés, dans l'ordre du registre.
#[must_use]
pub fn authorized_vaults(context: &Context, header: Option<&str>) -> Vec<Arc<VaultContext>> {
    let data = snapshots(context);
    let ids = authorized(header, &views(&data));
    context
        .vaults
        .list()
        .into_iter()
        .filter(|vault| ids.contains(&vault.id))
        .collect()
}

/// Choisit le coffre d'une requête agent.
///
/// Paramètres : le contexte, l'en-tête, le corps JSON (son `vaultId` choisit le coffre).
/// Retour : le coffre.
///
/// # Errors
///
/// `unauthorized`, `vault-required`, ou `vault-missing` si le coffre choisi n'existe plus.
pub fn select_vault(context: &Context, header: Option<&str>, body: &Value) -> Result<Arc<VaultContext>> {
    let requested = match body.get("vaultId") {
        None => None,
        Some(Value::String(text)) => Some(text.as_str()),
        Some(_) => Some(""),
    };
    let data = snapshots(context);
    let id = select(header, requested, &views(&data))?;
    context.vaults.get(Some(&id))
}

/// Refuse la requête si aucun coffre n'accepte le jeton.
///
/// Paramètres : le contexte, l'en-tête.
///
/// # Errors
///
/// `unauthorized`.
pub fn require_known(context: &Context, header: Option<&str>) -> Result<()> {
    if authorized_vaults(context, header).is_empty() {
        Err(ErrorCode::Unauthorized.into())
    } else {
        Ok(())
    }
}
