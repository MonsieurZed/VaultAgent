//! Routes `POST /api/agent/<route>` : jeton, méthode, corps JSON, choix du coffre, puis la route elle-même. L'ordre
//! des contrôles est celui de `linux/http.mjs`.

use serde_json::Value;
use vaultagent_core::ErrorCode;

use crate::agent_auth::{require_known, select_vault};
use crate::agent_read::{catalog, sharing, write_preview};
use crate::agent_seal::seal_field;
use crate::agent_share::share;
use crate::agent_status::{lock, status, vaults};
use crate::agent_unlock::{
    approval_context, decide_unlock, elevation_begin, elevation_enroll, hello_begin, hello_enroll, request_unlock,
};
use crate::agent_write::write;
use crate::context::Context;
use crate::error::Result;
use crate::guard::Guard;
use crate::http::{Request, Response};

const BODY_LIMIT: usize = 24_576;

/// Lit le corps JSON : type déclaré, taille, syntaxe, dans cet ordre.
///
/// Paramètres : la requête.
/// Retour : la valeur JSON.
///
/// # Errors
///
/// `json-required`, `request-size`, `request-refused` (JSON invalide).
pub fn parse_body(request: &Request) -> Result<Value> {
    let declared = request.header("content-type").unwrap_or_default().to_ascii_lowercase();
    let json = declared
        .strip_prefix("application/json")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(';'));
    if !json {
        return Err(ErrorCode::JsonRequired.into());
    }
    if request.body.len() > BODY_LIMIT {
        return Err(ErrorCode::RequestSize.into());
    }
    serde_json::from_slice(&request.body).map_err(|_| ErrorCode::RequestRefused.into())
}

/// Traite une requête agent déjà passée par les contrôles de proxy et d'origine.
///
/// Paramètres : le contexte, la requête, le nom de la route (après `/api/agent/`).
/// Retour : la réponse.
///
/// # Errors
///
/// `unauthorized`, `method-refused`, les erreurs du corps, `vault-required`, `route-refused`, ou l'erreur de la route.
pub fn handle(context: &Context, request: &Request, name: &str) -> Result<Response> {
    let header = request.header("authorization");
    require_known(context, header)?;
    if request.method != "POST" {
        return Err(ErrorCode::MethodRefused.into());
    }
    let body = parse_body(request)?;
    if name == "vaults" {
        return Ok(vaults(context, header));
    }
    if body.is_null() {
        return Err(ErrorCode::RequestRefused.into());
    }
    let vault = select_vault(context, header, &body)?;
    let guard = Guard::begin(&vault, header)?;
    let device = guard.authorize()?;
    match name {
        "status" => status(context, &vault, &device),
        "request-unlock" => request_unlock(context, &vault, &device),
        "decide-unlock" => decide_unlock(context, &vault, &device, &body),
        "approval-context" => approval_context(context, &vault, &device),
        "hello-begin" => hello_begin(context, &vault, &device),
        "hello-enroll" => hello_enroll(context, &vault, &device, &body),
        "elevation-begin" => elevation_begin(context, &vault, &device),
        "elevation-enroll" => elevation_enroll(context, &vault, &device, &body),
        "catalog" => {
            let _serial = vault.session.serial();
            catalog(&guard, &body)
        }
        "seal" => {
            let _serial = vault.session.serial();
            seal_field(context, &guard, &body)
        }
        "write-preview" => {
            let _serial = vault.session.serial();
            write_preview(&guard, &body)
        }
        "write" => {
            let _serial = vault.session.serial();
            write(context, &vault, &guard, &body)
        }
        "sharing" => {
            let _serial = vault.session.serial();
            sharing(&guard)
        }
        "share" => {
            let _serial = vault.session.serial();
            share(context, &vault, &guard, &body)
        }
        "lock" => Ok(lock(&vault)),
        _ => Err(ErrorCode::RouteRefused.into()),
    }
}
