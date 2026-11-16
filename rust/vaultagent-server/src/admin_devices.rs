//! Routes d'administration des appareils : `auto-unlock`, `pairing`, `pair-decide` et `revoke`.

use serde::Serialize;
use serde_json::Value;
use vaultagent_core::device_list::{revoke as without_device, set_automatic};
use vaultagent_core::devices::enroll_named;
use vaultagent_core::{Enrolled, ErrorCode};

use crate::context::Context;
use crate::error::{Result, ServerError};
use crate::http::Response;
use crate::sync::locked;
use crate::vaults::VaultContext;

/// Réponse de `auto-unlock` et `pair-decide`.
#[derive(Serialize)]
struct Saved {
    saved: bool,
}

/// Réponse de `revoke`.
#[derive(Serialize)]
struct Revoked {
    revoked: bool,
}

/// Réponse de `pairing` : le code est préfixé par l'identifiant du coffre.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Issued {
    code: String,
    expires_in_seconds: u64,
}

/// Texte d'un champ, vide s'il manque ou n'est pas une chaîne.
fn text<'a>(body: &'a Value, name: &str) -> &'a str {
    body.get(name).and_then(Value::as_str).unwrap_or_default()
}

/// Profil du compte du coffre.
fn profile_of(vault: &VaultContext) -> Option<String> {
    vault.store.account().map(|account| account.profile)
}

/// `auto-unlock` : active ou coupe l'acceptation automatique d'un appareil.
///
/// Paramètres : le coffre, le corps (`deviceId`, `enabled` booléen).
/// Retour : `{saved:true}`.
///
/// # Errors
///
/// `invalid-device` si `enabled` n'est pas booléen ou si l'appareil n'existe pas ; erreur d'écriture.
pub fn auto_unlock(vault: &VaultContext, body: &Value) -> Result<Response> {
    let enabled = body
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or(ErrorCode::InvalidDevice)?;
    let profile = profile_of(vault).unwrap_or_default();
    vault.store.update_devices(|records| {
        set_automatic(records, text(body, "deviceId"), &profile, enabled).map_err(ServerError::from)
    })?;
    Ok(Response::json(200, &Saved { saved: true }))
}

/// `pairing` : émet un code d'association préfixé par l'identifiant du coffre.
///
/// Paramètres : le contexte, le coffre, le corps (`name`).
/// Retour : `{code:"<coffre>.<code>", expiresInSeconds}`.
///
/// # Errors
///
/// `not-configured`, `invalid-device`, `pairing-limit`.
pub fn pairing(context: &Context, vault: &VaultContext, body: &Value) -> Result<Response> {
    let profile = profile_of(vault).ok_or(ErrorCode::NotConfigured)?;
    let issued = locked(&vault.devices).issue(
        text(body, "name"),
        &profile,
        context.clock.wall_ms(),
        &mut context.entropy.clone(),
    )?;
    Ok(Response::json(
        200,
        &Issued {
            code: format!("{}.{}", vault.id, issued.code),
            expires_in_seconds: issued.expires_in_seconds,
        },
    ))
}

/// `pair-decide` : approuve (`approve` vaut exactement `true`) ou refuse une demande d'association.
///
/// Paramètres : le contexte, le coffre qui recevra l'appareil, le corps (`requestId`, `approve`).
/// Retour : `{saved:true}`.
///
/// # Errors
///
/// `not-configured` (approbation sans compte), `request-expired`, `device-limit`, erreur d'écriture.
pub fn pair_decide(context: &Context, vault: &VaultContext, body: &Value) -> Result<Response> {
    let id = text(body, "requestId");
    let now = context.clock.wall_ms();
    if body.get("approve") != Some(&Value::Bool(true)) {
        locked(&context.pair_requests).deny(id)?;
        return Ok(Response::json(200, &Saved { saved: true }));
    }
    let profile = profile_of(vault).ok_or(ErrorCode::NotConfigured)?;
    let mut requests = locked(&context.pair_requests);
    let target = requests.target(id, now)?;
    let mut entropy = context.entropy.clone();
    let _serial = locked(&vault.devices);
    let mut enrolled: Option<Enrolled> = None;
    vault.store.update_devices(|records| {
        let (next, created) = enroll_named(
            &target.name,
            Some(&profile),
            target.approver_public_key.as_deref(),
            now,
            records,
            &mut entropy,
        )?;
        enrolled = Some(created);
        Ok(next)
    })?;
    let created = enrolled.ok_or(ErrorCode::RequestExpired)?;
    requests.approve(id, now, &vault.id, &created)?;
    Ok(Response::json(200, &Saved { saved: true }))
}

/// `revoke` : verrouille le coffre puis retire l'appareil.
///
/// Paramètres : le coffre, le corps (`deviceId`).
/// Retour : `{revoked:true}`.
///
/// # Errors
///
/// Erreur d'écriture de `devices.json`.
pub fn revoke(vault: &VaultContext, body: &Value) -> Result<Response> {
    vault.session.lock();
    vault
        .store
        .update_devices(|records| Ok(without_device(records, text(body, "deviceId"))))?;
    Ok(Response::json(200, &Revoked { revoked: true }))
}
