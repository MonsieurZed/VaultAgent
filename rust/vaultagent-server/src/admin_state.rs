//! État administrateur d'un coffre : construit une fois, rendu en JSON par `GET /api/admin/state` et en HTML par le
//! front.

use serde::Serialize;
use vaultagent_core::device_list::list;
use vaultagent_core::unlock_replies::ListedRequest;
use vaultagent_core::{Listed, PublicDevice};

use crate::agent_unlock::with_env;
use crate::context::Context;
use crate::error::Result;
use crate::http::{Request, Response};
use crate::query::query_param;
use crate::session::SessionStatus;
use crate::sync::locked;
use crate::vaults::VaultContext;

/// Compte affiché dans l'état : jamais le mot de passe ni le profil.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub server: String,
    pub email: String,
    pub timeout_minutes: Option<u32>,
}

/// Réponse de `GET /api/admin/state`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub vault_id: String,
    pub vault_name: String,
    #[serde(flatten)]
    pub session: SessionStatus,
    pub account: Option<AccountView>,
    pub devices: Vec<PublicDevice>,
    pub unlock_requests: Vec<ListedRequest>,
    pub pair_requests: Vec<Listed>,
}

/// Construit l'état d'un coffre.
///
/// Paramètres : le contexte, le coffre.
/// Retour : l'état.
///
/// # Errors
///
/// Erreur de lecture de la politique d'approbation.
pub fn snapshot(context: &Context, vault: &VaultContext) -> Result<State> {
    let account = vault.store.account();
    let unlock_requests = with_env(context, vault, |approvals, env| Ok(approvals.list(env)))?;
    let devices = account
        .as_ref()
        .map_or_default(|value| list(&vault.store.devices(), &value.profile));
    let pair_requests = locked(&context.pair_requests).list(context.clock.wall_ms());
    Ok(State {
        vault_id: vault.id.clone(),
        vault_name: vault.name(),
        session: vault.session.status(),
        account: account.map(|value| AccountView {
            server: value.server,
            email: value.email,
            timeout_minutes: value.timeout_minutes,
        }),
        devices,
        unlock_requests,
        pair_requests,
    })
}

/// `GET /api/admin/state?vaultId=` : état complet d'un coffre.
///
/// Paramètres : le contexte, la requête.
/// Retour : l'état.
///
/// # Errors
///
/// `vault-missing`, ou une erreur de lecture de la politique d'approbation.
pub fn state(context: &Context, request: &Request) -> Result<Response> {
    let requested = query_param(&request.path, "vaultId").filter(|id| !id.is_empty());
    let vault = context.vaults.get(requested.as_deref())?;
    Ok(Response::json(200, &snapshot(context, &vault)?))
}
