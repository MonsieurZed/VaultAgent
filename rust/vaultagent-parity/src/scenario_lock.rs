//! Cycle de verrouillage : `lock`, lecture refusée, demande de déverrouillage, décision signée par l'approbateur.

use crate::run::{Run, field, object};
use serde_json::Value;
use vaultagent_core::sign_decision;

/// Joue le verrouillage puis le déverrouillage demandé par le client et approuvé avec la clé de test.
///
/// Paramètres : le passage, associé et déverrouillé.
/// Retour : rien.
pub fn cycle(run: &mut Run<'_>) {
    run.agent_post("agent-lock", "lock", &object([]));
    run.agent_post("agent-status-locked", "status", &object([]));
    run.agent_post("agent-catalog-locked", "catalog", &object([]));
    let requested = run.agent_post("agent-request-unlock", "request-unlock", &object([]));
    let request_id = field(&requested, "requestId");
    let status = run.agent_post("agent-status-pending", "status", &object([]));
    let challenge = status
        .pointer("/approvalRequests/0/challenge")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let context = (
        run.origin.as_str(),
        run.vault_id.as_str(),
        request_id.as_str(),
        challenge.as_str(),
    );
    let signature = sign_decision(&run.settings.key, context, true, false, &mut run.entropy).unwrap_or_default();
    let decision = object([
        ("requestId", Value::from(request_id.as_str())),
        ("approve", Value::Bool(true)),
        ("remember", Value::Bool(false)),
        ("signature", Value::from(signature)),
    ]);
    run.agent_post("agent-decide-unlock", "decide-unlock", &decision);
    run.agent_post("agent-status-decided", "status", &object([]));
    run.agent_post("agent-approval-context", "approval-context", &object([]));
    run.agent_post("agent-request-unlock-unlocked", "request-unlock", &object([]));
}
