//! Association par approbation web : ouverture d'une demande, code de vérification sur stderr, interrogation.

use std::io::Write;
use std::time::Duration;

use crate::api::pair_call;
use crate::error::ClientError;
use crate::pairing_wire::{Opened, PairBody, Polled};
use crate::pause::Pause;
use crate::server_address::ServerAddress;
use crate::token::Token;
use crate::transport::Transport;

/// Intervalle entre deux interrogations.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Résultat d'une association approuvée.
#[derive(Debug)]
pub struct Paired {
    /// Jeton de l'appareil.
    pub token: Token,
    /// Coffre associé, si le serveur l'a donné.
    pub vault_id: Option<String>,
}

/// Décode une réponse JSON du serveur.
fn decode<'a, T: serde::Deserialize<'a>>(text: &'a str) -> Result<T, ClientError> {
    serde_json::from_str(text).map_err(|_| ClientError::BadResponse("Unexpected pairing answer.".to_owned()))
}

/// Écrit les consignes d'approbation.
fn announce(notice: &mut dyn Write, server: &ServerAddress, opened: &Opened) {
    writeln!(
        notice,
        "Verification code: {}\nApprove this request on {} (Clients tab) after checking the same code. Expires in {} seconds.",
        opened.verification,
        server.as_str(),
        opened.expires_in_seconds
    )
    .unwrap_or_default();
}

/// Interprète une interrogation.
fn outcome(polled: &Polled) -> Result<Option<Paired>, ClientError> {
    match polled.state.as_str() {
        "pending" => Ok(None),
        "denied" => Err(ClientError::PairingDenied),
        "approved" => {
            let token = polled
                .token
                .as_deref()
                .ok_or_else(|| ClientError::BadResponse("Approved pairing without token.".to_owned()))?;
            Ok(Some(Paired {
                token: Token::parse(token)
                    .map_err(|_| ClientError::BadResponse("Server sent an invalid token.".to_owned()))?,
                vault_id: polled.vault_id.clone(),
            }))
        }
        _ => Err(ClientError::BadResponse("Unknown pairing state.".to_owned())),
    }
}

/// Ouvre une demande d'association et attend l'approbation.
///
/// Paramètres : le transport vers le serveur, son adresse (pour les consignes), le nom de l'appareil, la clé publique
/// d'approbateur à enregistrer (ou `None`), l'attente injectée, la sortie des consignes (stderr).
/// Retour : le jeton et le coffre ; `PairingDenied` si l'administrateur refuse, `PairingExpired` si la durée de la
/// demande s'écoule sans réponse (comptée en pas de 2 s), ou l'erreur du serveur ou du transport.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn pair_request(
    transport: &dyn Transport,
    server: &ServerAddress,
    name: &str,
    approver_public_key: Option<&str>,
    pause: &mut dyn Pause,
    notice: &mut dyn Write,
) -> Result<Paired, ClientError> {
    let text = pair_call(
        transport,
        &PairBody::Request {
            name,
            approver_public_key,
        },
    )?;
    let opened: Opened = decode(&text)?;
    announce(notice, server, &opened);
    let limit = Duration::from_secs(opened.expires_in_seconds);
    let mut elapsed = Duration::ZERO;
    while elapsed < limit {
        pause.pause(POLL_INTERVAL);
        elapsed += POLL_INTERVAL;
        let body = PairBody::Poll {
            request_id: &opened.request_id,
            secret: &opened.secret,
        };
        let answer = pair_call(transport, &body)?;
        if let Some(paired) = outcome(&decode::<Polled>(&answer)?)? {
            return Ok(paired);
        }
    }
    Err(ClientError::PairingExpired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_transport::FakeTransport;
    use crate::pause::RecordedPause;
    use crate::test_dir::sample_token;

    const OPENED: &str =
        "{\"requestId\":\"r1\",\"secret\":\"fake-secret\",\"verification\":\"ABCD-EFGH\",\"expiresInSeconds\":6}";

    /// Aide de test.
    fn run(transport: &FakeTransport) -> (Result<Paired, ClientError>, RecordedPause, String) {
        let server = ServerAddress::parse("https://vault.example.test").unwrap();
        let mut pause = RecordedPause::default();
        let mut notice = Vec::new();
        let result = pair_request(transport, &server, "Machine", None, &mut pause, &mut notice);
        (result, pause, String::from_utf8(notice).unwrap())
    }

    #[test]
    fn approved_request_returns_the_token_after_polling() {
        let approved = format!(
            "{{\"state\":\"approved\",\"token\":\"{}\",\"deviceId\":\"d\",\"vaultId\":\"default\"}}",
            sample_token()
        );
        let transport = FakeTransport::new()
            .reply(200, OPENED)
            .reply(200, "{\"state\":\"pending\",\"expiresInSeconds\":4}")
            .reply(200, &approved);
        let (result, pause, notice) = run(&transport);
        let paired = result.unwrap();
        assert_eq!(paired.token.expose(), sample_token());
        assert_eq!(paired.vault_id.as_deref(), Some("default"));
        assert_eq!(pause.durations, vec![POLL_INTERVAL, POLL_INTERVAL]);
        assert!(notice.contains("ABCD-EFGH"));
        assert!(!notice.contains("fake-secret"));
        let calls = transport.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].body, "{\"action\":\"request\",\"name\":\"Machine\"}");
        assert_eq!(
            calls[1].body,
            "{\"action\":\"poll\",\"requestId\":\"r1\",\"secret\":\"fake-secret\"}"
        );
    }

    #[test]
    fn denied_request_is_refused() {
        let transport = FakeTransport::new()
            .reply(200, OPENED)
            .reply(200, "{\"state\":\"denied\"}");
        let (result, _, _) = run(&transport);
        assert_eq!(result.unwrap_err(), ClientError::PairingDenied);
    }

    #[test]
    fn expired_request_stops_after_the_validity_window() {
        let pending = "{\"state\":\"pending\",\"expiresInSeconds\":1}";
        let transport = FakeTransport::new()
            .reply(200, OPENED)
            .reply(200, pending)
            .reply(200, pending)
            .reply(200, pending);
        let (result, pause, _) = run(&transport);
        assert_eq!(result.unwrap_err(), ClientError::PairingExpired);
        assert_eq!(pause.durations.len(), 3);
    }

    #[test]
    fn server_refusal_while_opening_is_relayed() {
        let transport = FakeTransport::new().reply(400, "{\"error\":\"pairing-rate-limit\"}");
        let (result, _, _) = run(&transport);
        assert_eq!(result.unwrap_err().code(), "pairing-rate-limit");
    }

    #[test]
    fn invalid_token_from_the_server_is_refused() {
        let transport = FakeTransport::new()
            .reply(200, OPENED)
            .reply(200, "{\"state\":\"approved\",\"token\":\"nope\"}");
        let (result, _, _) = run(&transport);
        assert_eq!(result.unwrap_err().code(), "bad-response");
    }
}
