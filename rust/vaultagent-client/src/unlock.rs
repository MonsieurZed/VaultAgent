//! Commandes `unlock` (demande et attente bornée) et `lock`, selon l'algorithme de `agent-tools.mjs`.

use std::time::Duration;

use serde_json::{Map, Value};

use crate::api::{VaultBody, agent_call};
use crate::error::ClientError;
use crate::exit_code::ExitCode;
use crate::pause::Pause;
use crate::token::Token;
use crate::transport::Transport;

/// Attente maximale acceptée, en secondes.
pub const MAX_WAIT_SECONDS: i64 = 300;

/// Intervalle entre deux interrogations de `status`.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Résultat d'une commande : ligne JSON de stdout et code de sortie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Ligne JSON écrite sur stdout.
    pub line: String,
    /// Code de sortie du processus.
    pub exit: ExitCode,
}

/// Lit un corps de réponse comme objet JSON.
fn object(text: &str) -> Result<Map<String, Value>, ClientError> {
    match serde_json::from_str(text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(ClientError::BadResponse(
            "Server answered with an unexpected body.".to_owned(),
        )),
    }
}

/// Texte d'un champ de la réponse, vide s'il manque.
fn text<'a>(map: &'a Map<String, Value>, key: &str) -> &'a str {
    map.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Fixe `unlocked` et, si demandé, `state`, puis classe la sortie.
fn finish(mut map: Map<String, Value>, state: Option<&str>, unlocked: bool) -> Result<Outcome, ClientError> {
    if let Some(state) = state {
        map.insert("state".to_owned(), Value::String(state.to_owned()));
    }
    map.insert("unlocked".to_owned(), Value::Bool(unlocked));
    classify(map)
}

/// Sérialise une réponse et en déduit le code de sortie : 0 déverrouillé, 2 refusé ou en échec, 4 sinon.
fn classify(map: Map<String, Value>) -> Result<Outcome, ClientError> {
    let exit = if map.get("unlocked") == Some(&Value::Bool(true)) {
        ExitCode::Success
    } else if matches!(text(&map, "state"), "denied" | "failed") {
        ExitCode::Refused
    } else {
        ExitCode::Locked
    };
    let line = serde_json::to_string(&Value::Object(map))
        .map_err(|_| ClientError::BadResponse("Output not encodable.".to_owned()))?;
    Ok(Outcome { line, exit })
}

/// Résultat d'une interrogation de `status` pendant l'attente, `None` si la demande est encore en cours.
fn poll_result(current: &Map<String, Value>, request_id: Option<&Value>) -> Result<Option<Outcome>, ClientError> {
    let approval = current.get("unlockRequest").and_then(Value::as_object);
    let same = approval.filter(|approval| approval.get("requestId") == request_id);
    if let Some(approval) = same.filter(|approval| matches!(text(approval, "state"), "denied" | "failed")) {
        return finish(approval.clone(), None, false).map(Some);
    }
    if current.get("unlocked") == Some(&Value::Bool(true)) {
        let mut map = Map::new();
        map.insert("requestId".to_owned(), request_id.cloned().unwrap_or(Value::Null));
        return finish(map, Some("unlocked"), true).map(Some);
    }
    if same.is_none() {
        let mut map = Map::new();
        map.insert("requestId".to_owned(), request_id.cloned().unwrap_or(Value::Null));
        return finish(map, Some("expired"), false).map(Some);
    }
    Ok(None)
}

/// Demande le déverrouillage puis attend, sans jamais refaire de demande après un refus ou une expiration.
///
/// Paramètres : le transport, le jeton, le coffre, l'attente en secondes (0 à 300), l'attente injectée.
/// Retour : la sortie JSON et le code (0 déverrouillé, 4 si l'attente s'achève verrouillée, 2 refus ou échec) ;
/// `Usage` si l'attente est hors bornes (rien n'est envoyé) ; sinon l'erreur classée d'un appel.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn unlock(
    transport: &dyn Transport,
    token: &Token,
    vault: Option<&str>,
    wait_seconds: i64,
    pause: &mut dyn Pause,
) -> Result<Outcome, ClientError> {
    if !(0..=MAX_WAIT_SECONDS).contains(&wait_seconds) {
        return Err(ClientError::Usage("Wait must be between 0 and 300 seconds.".to_owned()));
    }
    let body = VaultBody { vault_id: vault };
    let result = object(&agent_call(transport, token, "request-unlock", &body)?)?;
    let waiting = matches!(text(&result, "state"), "pending" | "approving");
    if result.get("unlocked") == Some(&Value::Bool(true)) || wait_seconds == 0 || !waiting {
        return classify(result);
    }
    let total = Duration::from_secs(u64::try_from(wait_seconds).unwrap_or(0));
    let mut elapsed = Duration::ZERO;
    while elapsed < total {
        let step = POLL_INTERVAL.min(total.saturating_sub(elapsed));
        pause.pause(step);
        elapsed += step;
        let current = object(&agent_call(transport, token, "status", &body)?)?;
        if let Some(done) = poll_result(&current, result.get("requestId"))? {
            return Ok(done);
        }
    }
    finish(result, Some("waiting"), false)
}

/// Verrouille le coffre.
///
/// Paramètres : le transport, le jeton, le coffre.
/// Retour : le corps du serveur (`{"locked":true}`), ou l'erreur classée.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn lock(transport: &dyn Transport, token: &Token, vault: Option<&str>) -> Result<String, ClientError> {
    agent_call(transport, token, "lock", &VaultBody { vault_id: vault })
}
