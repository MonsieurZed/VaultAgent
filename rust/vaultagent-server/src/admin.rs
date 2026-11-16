//! Routes `/api/admin/*` du propriétaire : état du coffre (`GET`) et actions de session et d'écriture (`POST`).
//! Les actions sur les appareils et les demandes d'association sont dans `admin_devices`.

use std::sync::Arc;

use serde_json::Value;
use vaultagent_core::ErrorCode;
use zeroize::Zeroizing;

use crate::admin_devices::{auto_unlock, pair_decide, pairing, revoke};
use crate::agent::parse_body;
use crate::agent_read::write_preview;
use crate::agent_status::lock;
use crate::agent_unlock::with_env;
use crate::agent_write::write;
use crate::configure_input::ConfigureInput;
use crate::context::Context;
use crate::error::Result;
use crate::guard::Guard;
use crate::http::{Request, Response};
use crate::vaults::VaultContext;

/// Coffre désigné par `vaultId` : absent, il s'agit du coffre `default`.
///
/// Un `vaultId` qui n'est pas une chaîne désigne un coffre inconnu, comme dans Node.
fn pick(context: &Context, requested: Option<&Value>) -> Result<Arc<VaultContext>> {
    match requested {
        None => context.vaults.get(None),
        Some(Value::String(id)) => context.vaults.get(Some(id)),
        Some(_) => Err(ErrorCode::VaultMissing.into()),
    }
}

/// Action `POST` d'administration JSON : corps, puis la route.
///
/// Paramètres : le contexte, la requête (origine déjà contrôlée), le chemin.
/// Retour : la réponse.
///
/// # Errors
///
/// Erreurs du corps, `vault-missing`, `route-refused` ou l'erreur de la route.
pub fn post(context: &Context, request: &Request, path: &str) -> Result<Response> {
    let body = parse_body(request)?;
    if body.is_null() {
        return Err(ErrorCode::RequestRefused.into());
    }
    let name = path.strip_prefix("/api/admin/").unwrap_or_default();
    act(context, name, &body)
}

/// Exécute une action d'administration à partir de son corps déjà lu ; partagée par les routes JSON et les
/// formulaires du front.
///
/// Paramètres : le contexte, le nom de l'action, le corps JSON (objet).
/// Retour : la réponse de l'action.
///
/// # Errors
///
/// `vault-missing`, `route-refused` ou l'erreur de l'action.
pub fn act(context: &Context, name: &str, body: &Value) -> Result<Response> {
    let vault = pick(context, body.get("vaultId"))?;
    match name {
        "write-preview" => {
            let _serial = vault.session.serial();
            write_preview(&Guard::owner(&vault), body)
        }
        "write" => {
            let _serial = vault.session.serial();
            write(context, &vault, &Guard::owner(&vault), body)
        }
        "configure" => configure(context, &vault, body),
        "unlock" => unlock(&vault, body),
        "decide-unlock" => decide_unlock(context, &vault, body),
        "lock" => Ok(lock(&vault)),
        "auto-unlock" => auto_unlock(&vault, body),
        "pairing" => pairing(context, &vault, body),
        "pair-decide" => pair_decide(context, &vault, body),
        "revoke" => revoke(&vault, body),
        _ => Err(ErrorCode::RouteRefused.into()),
    }
}

/// `configure` : enregistre le compte et ouvre la session.
fn configure(context: &Context, vault: &VaultContext, body: &Value) -> Result<Response> {
    let status = context.vaults.configure(vault, &ConfigureInput::from_json(body))?;
    Ok(Response::json(200, &status))
}

/// `unlock` : ouvre la session avec le code de double authentification éventuel.
fn unlock(vault: &VaultContext, body: &Value) -> Result<Response> {
    let code = body
        .get("code")
        .and_then(Value::as_str)
        .map(|text| Zeroizing::new(text.to_owned()));
    Ok(Response::json(200, &vault.session.unlock(code.as_ref())?))
}

/// `decide-unlock` : accepte ou refuse la demande d'un client ; un `approve` qui n'est pas booléen la fait expirer.
fn decide_unlock(context: &Context, vault: &VaultContext, body: &Value) -> Result<Response> {
    let id = body.get("requestId").and_then(Value::as_str);
    let approve = body.get("approve").and_then(Value::as_bool);
    let code = body.get("code").and_then(Value::as_str);
    let (Some(id), Some(approve)) = (id, approve) else {
        return Err(ErrorCode::RequestExpired.into());
    };
    let decided = with_env(context, vault, |approvals, env| {
        Ok(approvals.decide(id, approve, code, env)?)
    })?;
    Ok(Response::json(200, &decided))
}
