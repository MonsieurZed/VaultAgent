//! Routes de déverrouillage d'un client agent : demande, décision d'un approbateur, contextes signés et enrôlements
//! Hello et élévation. La logique est celle de `vaultagent-core::UnlockRequests` ; ce module fournit l'environnement
//! (session, aléa, heure, appareils, politique) et enregistre les mutations d'appareils et de politique.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use vaultagent_core::{ElevationEnrollInput, Env, HelloEnrollInput, Outcome, SignedDecisionInput, UnlockRequests};

use crate::context::Context;
use crate::error::Result;
use crate::http::Response;
use crate::session::Session;
use crate::shared_entropy::SharedEntropy;
use crate::sync::locked;
use crate::vaults::VaultContext;

/// Exécute une opération sur les demandes du coffre avec l'environnement courant.
///
/// Paramètres : le contexte, le coffre, l'opération qui reçoit les demandes et l'environnement.
/// Retour : le résultat de l'opération.
///
/// # Errors
///
/// Erreur de lecture de la politique d'approbation, ou celle de l'opération.
pub fn with_env<T>(
    context: &Context,
    vault: &VaultContext,
    operation: impl FnOnce(&mut UnlockRequests, &mut Env<'_, &Session, SharedEntropy>) -> Result<T>,
) -> Result<T> {
    let records = vault.store.devices();
    let policy = vault.store.approval_policy()?;
    let mut session: &Session = &vault.session;
    let mut entropy = context.entropy.clone();
    let mut env = Env {
        session: &mut session,
        entropy: &mut entropy,
        now: context.clock.wall_ms(),
        records: &records,
        policy: &policy,
    };
    let mut approvals = locked(&vault.approvals);
    operation(&mut approvals, &mut env)
}

/// Enregistre les appareils et la politique modifiés, puis rend la réponse publique.
fn persist<T>(vault: &VaultContext, outcome: Outcome<T>) -> Result<T> {
    if let Some(devices) = outcome.devices {
        vault.store.update_devices(|_| Ok(devices))?;
    }
    if let Some(policy) = outcome.policy {
        vault.store.save_approval_policy(&policy)?;
    }
    Ok(outcome.value)
}

/// Lit le corps comme une structure d'entrée ; un champ d'un autre type vaut un champ absent, que la logique
/// du coeur refuse avec son propre code (`hello-enrollment-refused`, `approver-required`, …).
fn input<T: DeserializeOwned + Default>(body: &Value) -> T {
    serde_json::from_value(body.clone()).unwrap_or_default()
}

/// Réponse JSON de statut 200.
fn reply<T: Serialize>(value: &T) -> Response {
    Response::json(200, value)
}

/// `request-unlock` : crée une demande de déverrouillage, ou ouvre le coffre si l'appareil l'accepte d'avance.
///
/// Paramètres : le contexte, le coffre, l'appareil authentifié.
/// Retour : la réponse du protocole.
///
/// # Errors
///
/// `not-configured`, `unauthorized`, `request-expired` ou l'erreur d'ouverture de la session.
pub fn request_unlock(context: &Context, vault: &VaultContext, device: &str) -> Result<Response> {
    let answer = with_env(context, vault, |approvals, env| Ok(approvals.request(device, env)?))?;
    Ok(reply(&answer))
}

/// `decide-unlock` : décision signée d'un approbateur, ou retrait de l'acceptation automatique.
///
/// Paramètres : le contexte, le coffre, l'appareil, le corps.
/// Retour : la demande décidée ou `{automaticAcceptance:false}`.
///
/// # Errors
///
/// `approver-required`, `request-expired`, `approval-refused`, `hello-*`, `elevation-required`.
pub fn decide_unlock(context: &Context, vault: &VaultContext, device: &str, body: &Value) -> Result<Response> {
    let decision: SignedDecisionInput = input(body);
    let outcome = with_env(context, vault, |approvals, env| {
        Ok(approvals.signed_decision(device, &decision, &context.origin, env)?)
    })?;
    Ok(reply(&persist(vault, outcome)?))
}

/// `approval-context` : défi de `reset-auto`.
///
/// Paramètres : le contexte, le coffre, l'appareil.
/// Retour : `{vaultId, requestId, challenge}`.
///
/// # Errors
///
/// `approver-required`.
pub fn approval_context(context: &Context, vault: &VaultContext, device: &str) -> Result<Response> {
    let answer = with_env(context, vault, |approvals, env| Ok(approvals.context(device, env)?))?;
    Ok(reply(&answer))
}

/// `hello-begin` : ouvre l'enrôlement Windows Hello.
///
/// Paramètres : le contexte, le coffre, l'appareil.
/// Retour : le défi et le justificatif déjà enrôlé.
///
/// # Errors
///
/// `hello-enrollment-refused`, `hello-enrollment-authorization-required`.
pub fn hello_begin(context: &Context, vault: &VaultContext, device: &str) -> Result<Response> {
    let answer = with_env(context, vault, |approvals, env| Ok(approvals.begin_hello(device, env)?))?;
    Ok(reply(&answer))
}

/// `hello-enroll` : termine l'enrôlement Windows Hello.
///
/// Paramètres : le contexte, le coffre, l'appareil, le corps.
/// Retour : `{helloEnabled, helloRequired, vaultId}`.
///
/// # Errors
///
/// `hello-enrollment-refused`, `approval-refused`, `hello-required`, `hello-invalid`.
pub fn hello_enroll(context: &Context, vault: &VaultContext, device: &str, body: &Value) -> Result<Response> {
    let enrollment: HelloEnrollInput = input(body);
    let outcome = with_env(context, vault, |approvals, env| {
        Ok(approvals.complete_hello_enrollment(device, &enrollment, &context.origin, env)?)
    })?;
    Ok(reply(&persist(vault, outcome)?))
}

/// `elevation-begin` : ouvre l'enrôlement de la clé d'élévation.
///
/// Paramètres : le contexte, le coffre, l'appareil.
/// Retour : `{vaultId, requestId, challenge}`.
///
/// # Errors
///
/// `elevation-enrollment-refused`.
pub fn elevation_begin(context: &Context, vault: &VaultContext, device: &str) -> Result<Response> {
    let answer = with_env(context, vault, |approvals, env| {
        Ok(approvals.begin_elevation(device, env)?)
    })?;
    Ok(reply(&answer))
}

/// `elevation-enroll` : termine l'enrôlement de la clé d'élévation.
///
/// Paramètres : le contexte, le coffre, l'appareil, le corps.
/// Retour : `{elevationEnabled, vaultId}`.
///
/// # Errors
///
/// `elevation-enrollment-refused`, `approval-refused`, `invalid-key`, `elevation-required`.
pub fn elevation_enroll(context: &Context, vault: &VaultContext, device: &str, body: &Value) -> Result<Response> {
    let enrollment: ElevationEnrollInput = input(body);
    let outcome = with_env(context, vault, |approvals, env| {
        Ok(approvals.complete_elevation_enrollment(device, &enrollment, &context.origin, env)?)
    })?;
    Ok(reply(&persist(vault, outcome)?))
}
