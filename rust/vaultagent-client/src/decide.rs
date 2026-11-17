//! Décision d'une demande de déverrouillage : message signé par la clé d'approbateur, avec en plus la preuve
//! d'élévation et la preuve Hello quand la demande les exige (mêmes règles que `RemoteApprovals.cs`).

use serde::Serialize;
use serde_json::Value;
use vaultagent_core::decision_message;

use crate::api::agent_call;
use crate::approval_request::ApprovalRequest;
use crate::approver_key::ApproverKey;
use crate::config::ClientConfig;
use crate::error::ClientError;
use crate::proofs::Proofs;
use crate::transport::Transport;

/// Corps de `decide-unlock`.
#[derive(Serialize)]
struct DecideBody<'a> {
    #[serde(rename = "vaultId")]
    vault_id: &'a str,
    #[serde(rename = "requestId")]
    request_id: &'a str,
    approve: bool,
    remember: bool,
    signature: String,
    #[serde(rename = "helloProof", skip_serializing_if = "Option::is_none")]
    hello_proof: Option<Value>,
    #[serde(rename = "elevationSignature", skip_serializing_if = "Option::is_none")]
    elevation_signature: Option<String>,
}

/// Approbateur de ce poste : serveur, appareil associé et clé d'approbateur.
pub struct Approver<'a> {
    /// Transport vers le serveur.
    pub transport: &'a dyn Transport,
    /// Configuration (origine du serveur et jeton).
    pub config: &'a ClientConfig,
    /// Clé d'approbateur de ce poste.
    pub key: &'a ApproverKey,
}

impl Approver<'_> {
    /// Message signé d'une décision ou d'un enrôlement.
    ///
    /// Paramètres : le coffre, la demande, le défi, la décision et le souhait de mémoriser.
    /// Retour : le JSON compact `["VaultAgent.Unlock.v2", …]`.
    #[must_use]
    pub fn message(&self, vault: &str, request_id: &str, challenge: &str, approve: bool, remember: bool) -> String {
        decision_message(
            self.config.server.as_str(),
            vault,
            request_id,
            challenge,
            approve,
            remember,
        )
    }

    /// Décide une demande : signe, ajoute les preuves exigées si on approuve, puis appelle `decide-unlock`.
    ///
    /// Paramètres : la demande, `approve`, `remember` (ignoré si on refuse) et les preuves.
    /// Retour : le corps JSON du serveur ; un refus (`approve` faux) est signé sans preuve ni invite UAC ;
    /// `HelloUnavailable`, `ElevationCancelled` ou `Elevation` si une preuve exigée manque (rien n'est envoyé) ;
    /// sinon l'erreur classée de l'appel.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn decide_unlock(
        &self,
        request: &ApprovalRequest,
        approve: bool,
        remember: bool,
        proofs: &dyn Proofs,
    ) -> Result<String, ClientError> {
        let remember = approve && remember;
        let message = self.message(
            request.vault(),
            &request.request_id,
            &request.challenge,
            approve,
            remember,
        );
        let hello_proof = if approve && request.hello_required {
            let credential = request.hello_credential.as_ref().filter(|value| !value.is_null());
            Some(proofs.hello(Some(credential.ok_or(ClientError::HelloUnavailable)?), &message)?)
        } else {
            None
        };
        let elevation_signature = if approve && request.elevation_required {
            Some(proofs.elevation(&message)?.signature)
        } else {
            None
        };
        let body = DecideBody {
            vault_id: request.vault(),
            request_id: &request.request_id,
            approve,
            remember,
            signature: self.key.sign(&message)?,
            hello_proof,
            elevation_signature,
        };
        agent_call(self.transport, &self.config.token, "decide-unlock", &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_transport::FakeTransport;
    use crate::proofs::FakeProofs;
    use crate::protector::ReversingProtector;
    use crate::server_address::ServerAddress;
    use crate::test_dir::{TestDir, sample_token};
    use crate::token::Token;
    use vaultagent_core::{ErrorCode, verify_pss};

    const ANSWER: &str = "{\"vaultId\":\"default\",\"requestId\":\"r1\",\"state\":\"approved\"}";

    /// Configuration, clé d'approbateur et dossier de test.
    fn setup() -> (ClientConfig, ApproverKey, TestDir) {
        let dir = TestDir::new();
        let key = ApproverKey::load_or_create(&dir.path().join("k"), &ReversingProtector).unwrap();
        let config = ClientConfig {
            server: ServerAddress::parse("https://vault.example.test").unwrap(),
            token: Token::parse(&sample_token()).unwrap(),
        };
        (config, key, dir)
    }

    /// Demande factice avec les exigences données.
    fn request(hello: bool, elevation: bool) -> ApprovalRequest {
        ApprovalRequest {
            vault_id: None,
            request_id: "r1".to_owned(),
            challenge: "chal".to_owned(),
            hello_required: hello,
            elevation_required: elevation,
            hello_credential: hello.then(|| serde_json::json!({"id": "cred"})),
        }
    }

    /// Corps JSON de la première requête envoyée.
    fn sent(fake: &FakeTransport) -> Value {
        serde_json::from_str(&fake.calls()[0].body).unwrap()
    }

    #[test]
    fn approve_with_elevation_sends_both_signatures() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        let proofs = FakeProofs::granting();
        approver
            .decide_unlock(&request(false, true), true, true, &proofs)
            .unwrap();
        let body = sent(&fake);
        let message = decision_message("https://vault.example.test", "default", "r1", "chal", true, true);
        verify_pss(
            &key.public_key().unwrap(),
            &message,
            body["signature"].as_str().unwrap(),
            ErrorCode::InvalidKey,
        )
        .unwrap();
        assert_eq!(body["elevationSignature"], "sig-elev");
        assert_eq!(body["vaultId"], "default");
        assert_eq!(body["remember"], true);
        assert!(body.get("helloProof").is_none());
        assert_eq!(proofs.asked.borrow().as_slice(), [message]);
        assert_eq!(fake.calls()[0].path, "/api/agent/decide-unlock");
    }

    #[test]
    fn approve_without_requirement_has_no_proof_and_no_uac() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        let proofs = FakeProofs::refusing();
        approver
            .decide_unlock(&request(false, false), true, false, &proofs)
            .unwrap();
        let body = sent(&fake);
        assert_eq!(
            body.as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            ["approve", "remember", "requestId", "signature", "vaultId"]
                .map(String::from)
                .into()
        );
        assert_eq!(body["approve"], true);
        assert_eq!(proofs.asked.borrow().len(), 0);
    }

    #[test]
    fn denial_is_signed_without_uac_even_when_elevation_is_required() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        let proofs = FakeProofs::refusing();
        approver
            .decide_unlock(&request(true, true), false, true, &proofs)
            .unwrap();
        let body = sent(&fake);
        let message = decision_message("https://vault.example.test", "default", "r1", "chal", false, false);
        verify_pss(
            &key.public_key().unwrap(),
            &message,
            body["signature"].as_str().unwrap(),
            ErrorCode::InvalidKey,
        )
        .unwrap();
        assert_eq!(
            (body["approve"].clone(), body["remember"].clone()),
            (false.into(), false.into())
        );
        assert!(body.get("elevationSignature").is_none() && body.get("helloProof").is_none());
        assert_eq!(proofs.asked.borrow().len(), 0);
    }

    #[test]
    fn missing_proof_sends_nothing() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        let cancelled = approver.decide_unlock(&request(false, true), true, false, &FakeProofs::refusing());
        assert_eq!(cancelled.unwrap_err().code(), "elevation-cancelled");
        let hello = approver.decide_unlock(&request(true, false), true, false, &FakeProofs::refusing());
        assert_eq!(hello.unwrap_err().code(), "hello-unavailable");
        let mut without_credential = request(true, false);
        without_credential.hello_credential = None;
        let missing = approver.decide_unlock(&without_credential, true, false, &FakeProofs::granting());
        assert_eq!(missing.unwrap_err().code(), "hello-unavailable");
        assert_eq!(fake.calls().len(), 0);
    }

    #[test]
    fn hello_proof_is_added_when_required() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        approver
            .decide_unlock(&request(true, false), true, false, &FakeProofs::granting())
            .unwrap();
        assert_eq!(sent(&fake)["helloProof"]["proof"], "hello");
    }
}
