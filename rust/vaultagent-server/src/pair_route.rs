//! Route `POST /api/pair`, sans authentification : ouverture et sondage d'une demande d'association, ou code émis
//! par l'administrateur.

use serde::Serialize;
use serde_json::Value;
use vaultagent_core::Enrolled;

use crate::agent::parse_body;
use crate::context::Context;
use crate::error::Result;
use crate::http::{Request, Response};
use crate::sync::locked;

/// Réponse d'un code d'association consommé.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Paired<'a> {
    token: &'a str,
    device_id: &'a str,
    vault_id: &'a str,
}

/// Traite `POST /api/pair` après les contrôles de proxy et d'origine.
///
/// Paramètres : le contexte, la requête.
/// Retour : la réponse.
///
/// # Errors
///
/// Erreurs du corps, `invalid-device`, `invalid-key`, `pairing-rate-limit`, `pairing-limit`, `pairing-refused`,
/// `vault-missing`, `device-limit`.
pub fn handle(context: &Context, request: &Request) -> Result<Response> {
    let body = parse_body(request)?;
    if body.is_null() {
        return Err(vaultagent_core::ErrorCode::RequestRefused.into());
    }
    match body.get("action").and_then(Value::as_str) {
        Some("request") => open(context, &body),
        Some("poll") => poll(context, &body),
        _ => by_code(context, &body),
    }
}

/// Texte d'un champ, vide s'il manque ou n'est pas une chaîne.
fn text<'a>(body: &'a Value, name: &str) -> &'a str {
    body.get(name).and_then(Value::as_str).unwrap_or_default()
}

/// `{action:"request"}` : ouvre une demande.
///
/// Une clé d'approbateur qui n'est pas une chaîne est transmise comme chaîne vide, que la logique du coeur refuse
/// en `invalid-key` après avoir contrôlé le nom, comme Node.
fn open(context: &Context, body: &Value) -> Result<Response> {
    let key = body
        .get("approverPublicKey")
        .map(|value| value.as_str().unwrap_or_default());
    let opened = locked(&context.pair_requests).open(
        text(body, "name"),
        key,
        context.clock.wall_ms(),
        &mut context.entropy.clone(),
    )?;
    Ok(Response::json(200, &opened))
}

/// `{action:"poll"}` : état d'une demande.
fn poll(context: &Context, body: &Value) -> Result<Response> {
    let polled =
        locked(&context.pair_requests).poll(text(body, "requestId"), text(body, "secret"), context.clock.wall_ms())?;
    Ok(Response::json(200, &polled))
}

/// `{code}` ou `{code:"<coffre>.<code>"}` : consomme un code d'association émis par l'administrateur.
fn by_code(context: &Context, body: &Value) -> Result<Response> {
    let given = body.get("code").and_then(Value::as_str);
    let (vault_id, code) = match given.and_then(|text| text.split_once('.')) {
        Some((id, rest)) => (Some(id), Some(rest)),
        None => (None, given),
    };
    let vault = context.vaults.get(vault_id)?;
    let profile = vault.store.account().map_or_default(|account| account.profile);
    let now = context.clock.wall_ms();
    let mut entropy = context.entropy.clone();
    let mut devices = locked(&vault.devices);
    let mut enrolled: Option<Enrolled> = None;
    vault.store.update_devices(|records| {
        let (next, created) = devices.pair(code, &profile, now, records, &mut entropy)?;
        enrolled = Some(created);
        Ok(next)
    })?;
    let created = enrolled.ok_or(vaultagent_core::ErrorCode::PairingRefused)?;
    Ok(Response::json(
        200,
        &Paired {
            token: &created.token,
            device_id: &created.device_id,
            vault_id: &vault.id,
        },
    ))
}
