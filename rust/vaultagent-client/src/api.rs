//! Appels du protocole : envoi d'un corps JSON, classement du statut HTTP, relais du corps ou du code d'erreur.

use serde::Serialize;
use serde_json::Value;

use crate::error::ClientError;
use crate::token::Token;
use crate::transport::{PAIRING_TIMEOUT, REQUEST_TIMEOUT, Response, Transport, TransportError};

/// Longueur maximale d'un code d'erreur du serveur repris tel quel.
const MAX_CODE_LENGTH: usize = 80;

/// Code d'erreur repris quand le serveur n'en donne pas de valide.
const FALLBACK_CODE: &str = "request-refused";

/// Traduit un échec de transport en erreur du client.
///
/// Paramètres : l'échec.
/// Retour : `Unreachable` si rien n'est parti, `Indeterminate` après envoi, `BadResponse` sinon.
#[must_use]
pub fn map_transport(error: TransportError) -> ClientError {
    match error {
        TransportError::NotSent(text) => ClientError::Unreachable(text),
        TransportError::Indeterminate => ClientError::Indeterminate,
        TransportError::TooLarge => ClientError::BadResponse("Response larger than 4 MiB refused.".to_owned()),
        TransportError::Redirected => ClientError::BadResponse("Redirection refused.".to_owned()),
    }
}

/// Indique si un texte a la forme d'un code du protocole (`^[a-z-]+(?::[a-z-]+)?$`, éventuellement `engine-*`).
fn is_protocol_code(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_CODE_LENGTH
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-' || byte == b':')
}

/// Extrait le code d'erreur d'un corps `{"error": code}`.
fn error_code(body: &[u8]) -> String {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| value.get("error").and_then(Value::as_str).map(str::to_owned))
        .filter(|code| is_protocol_code(code))
        .unwrap_or_else(|| FALLBACK_CODE.to_owned())
}

/// Classe une réponse du serveur.
///
/// Paramètres : la réponse brute.
/// Retour : le corps JSON sur une ligne si le statut est 2xx ; sinon `Server {status, code}` (423 vaut
/// verrouillé) ; une redirection ou un corps non JSON donne `BadResponse`.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn classify(response: &Response) -> Result<String, ClientError> {
    if (300..400).contains(&response.status) {
        return Err(ClientError::BadResponse("Redirection refused.".to_owned()));
    }
    if !(200..300).contains(&response.status) {
        return Err(ClientError::Server {
            status: response.status,
            code: error_code(&response.body),
        });
    }
    let value: Value = serde_json::from_slice(&response.body)
        .map_err(|_| ClientError::BadResponse("Server answered with invalid JSON.".to_owned()))?;
    let text = String::from_utf8_lossy(&response.body).trim().to_owned();
    if text.contains('\n') {
        return serde_json::to_string(&value)
            .map_err(|_| ClientError::BadResponse("Response not encodable.".to_owned()));
    }
    Ok(text)
}

/// Encode un corps de requête.
fn encode(body: &impl Serialize) -> Result<Vec<u8>, ClientError> {
    serde_json::to_vec(body).map_err(|_| ClientError::Usage("Request is not encodable.".to_owned()))
}

/// Appelle une route agent avec le jeton de l'appareil.
///
/// Paramètres : le transport, le jeton, le nom de la route (`status`, `catalog`…), le corps.
/// Retour : le corps JSON de la réponse, ou l'erreur classée.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn agent_call(
    transport: &dyn Transport,
    token: &Token,
    route: &str,
    body: &impl Serialize,
) -> Result<String, ClientError> {
    agent_call_bytes(transport, token, route, &encode(body)?)
}

/// Corps d'une route qui ne porte que le coffre visé.
#[derive(Serialize)]
pub struct VaultBody<'a> {
    /// Coffre choisi par `--vault`.
    #[serde(rename = "vaultId", skip_serializing_if = "Option::is_none")]
    pub vault_id: Option<&'a str>,
}

/// Appelle une route agent avec un corps déjà encodé (le tampon reste à l'appelant, qui peut l'effacer).
///
/// Paramètres : le transport, le jeton, le nom de la route, les octets JSON du corps.
/// Retour : le corps JSON de la réponse, ou l'erreur classée.
///
/// # Errors
///
/// Les erreurs possibles sont celles de `agent_call`.
pub fn agent_call_bytes(
    transport: &dyn Transport,
    token: &Token,
    route: &str,
    body: &[u8],
) -> Result<String, ClientError> {
    let bearer = token.bearer();
    let response = transport
        .post(
            &format!("/api/agent/{route}"),
            Some(bearer.as_str()),
            body,
            REQUEST_TIMEOUT,
        )
        .map_err(map_transport)?;
    classify(&response)
}

/// Appelle `/api/pair`, sans authentification.
///
/// Paramètres : le transport, le corps (`request` ou `poll`).
/// Retour : le corps JSON de la réponse, ou l'erreur classée.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn pair_call(transport: &dyn Transport, body: &impl Serialize) -> Result<String, ClientError> {
    let response = transport
        .post("/api/pair", None, &encode(body)?, PAIRING_TIMEOUT)
        .map_err(map_transport)?;
    classify(&response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exit_code::ExitCode;
    use crate::fake_transport::FakeTransport;
    use crate::test_dir::sample_token;
    use serde_json::json;

    /// Aide de test.
    fn call(reply: FakeTransport) -> (Result<String, ClientError>, FakeTransport) {
        let token = Token::parse(&sample_token()).unwrap();
        let result = agent_call(&reply, &token, "status", &json!({"version": 1}));
        (result, reply)
    }

    #[test]
    fn success_relays_the_body_and_sends_the_bearer() {
        let (result, transport) = call(FakeTransport::new().reply(200, "{\"unlocked\":true}"));
        assert_eq!(result.unwrap(), "{\"unlocked\":true}");
        let sent = &transport.calls()[0];
        assert_eq!(sent.path, "/api/agent/status");
        assert_eq!(
            sent.bearer.as_deref(),
            Some(format!("Bearer {}", sample_token()).as_str())
        );
    }

    #[test]
    fn multiline_body_is_compacted() {
        let (result, _) = call(FakeTransport::new().reply(200, "{\n  \"a\": 1\n}\n"));
        assert_eq!(result.unwrap(), "{\"a\":1}");
    }

    #[test]
    fn statuses_map_to_exit_codes() {
        let cases = [
            (423, "{\"error\":\"locked\"}", ExitCode::Locked, "locked"),
            (
                400,
                "{\"error\":\"vault-required\"}",
                ExitCode::Refused,
                "vault-required",
            ),
            (403, "{\"error\":\"unauthorized\"}", ExitCode::Refused, "unauthorized"),
            (500, "oops <html>", ExitCode::Refused, "request-refused"),
            (
                400,
                "{\"error\":\"Bad Code With Spaces\"}",
                ExitCode::Refused,
                "request-refused",
            ),
            (400, "{\"error\":\"engine-locked\"}", ExitCode::Refused, "engine-locked"),
        ];
        for (status, body, exit, code) in cases {
            let error = call(FakeTransport::new().reply(status, body)).0.unwrap_err();
            assert_eq!(error.exit_code(), exit, "status {status}");
            assert_eq!(error.code(), code);
        }
    }

    #[test]
    fn redirect_and_invalid_json_are_refused() {
        assert_eq!(
            call(FakeTransport::new().reply(302, "")).0.unwrap_err().exit_code(),
            ExitCode::Refused
        );
        assert_eq!(
            call(FakeTransport::new().reply(200, "not json")).0.unwrap_err().code(),
            "bad-response"
        );
    }

    #[test]
    fn transport_failures_are_classified() {
        let timeout = call(FakeTransport::new().fail(TransportError::Indeterminate))
            .0
            .unwrap_err();
        assert_eq!(timeout.exit_code(), ExitCode::Indeterminate);
        let unreachable = call(FakeTransport::new().fail(TransportError::NotSent("x".to_owned())))
            .0
            .unwrap_err();
        assert_eq!(unreachable.exit_code(), ExitCode::Refused);
        assert_eq!(unreachable.code(), "unreachable");
        let large = call(FakeTransport::new().fail(TransportError::TooLarge)).0.unwrap_err();
        assert_eq!(large.code(), "bad-response");
    }

    #[test]
    fn pairing_calls_carry_no_bearer() {
        let transport = FakeTransport::new().reply(200, "{}");
        pair_call(&transport, &json!({"version": 1})).unwrap();
        let sent = &transport.calls()[0];
        assert_eq!(sent.path, "/api/pair");
        assert!(sent.bearer.is_none());
        assert_eq!(sent.timeout, PAIRING_TIMEOUT);
    }
}
