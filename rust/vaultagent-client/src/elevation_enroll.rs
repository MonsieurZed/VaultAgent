//! Commande `elevation-enroll` : enrôle la clé d'élévation de ce poste sur chaque coffre dont l'approbateur est
//! enregistré et qui n'a pas encore d'élévation (algorithme `EnrollElevationAsync` du client .NET).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::{VaultBody, agent_call};
use crate::approval_request::DEFAULT_VAULT;
use crate::decide::Approver;
use crate::error::ClientError;
use crate::proofs::Proofs;

/// Sortie de la commande : le nombre de coffres enrôlés.
#[derive(Serialize)]
struct Enrolled {
    enrolled: usize,
}

/// Coffre listé par la route `vaults`.
#[derive(Deserialize)]
pub(crate) struct VaultEntry {
    pub(crate) id: String,
}

/// Contexte d'enrôlement rendu par `elevation-begin`.
#[derive(Deserialize)]
struct Begin {
    #[serde(rename = "vaultId", default)]
    vault_id: Option<String>,
    #[serde(rename = "requestId")]
    request_id: String,
    challenge: String,
}

/// Corps de `elevation-enroll`.
#[derive(Serialize)]
struct EnrollBody {
    #[serde(rename = "vaultId")]
    vault_id: String,
    #[serde(rename = "publicKey")]
    public_key: String,
    signature: String,
    #[serde(rename = "elevationSignature")]
    elevation_signature: String,
}

/// Texte d'un champ booléen de `approvalSecurity`, faux s'il manque.
pub(crate) fn flag(security: &Value, key: &str) -> bool {
    security.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Décode un corps de réponse.
pub(crate) fn decode<'a, T: Deserialize<'a>>(text: &'a str) -> Result<T, ClientError> {
    serde_json::from_str(text)
        .map_err(|_| ClientError::BadResponse("Server answered with an unexpected body.".to_owned()))
}

impl Approver<'_> {
    /// Enrôle la clé d'élévation sur un coffre : défi, signature élevée, signature d'approbateur, enrôlement.
    fn enroll_vault(&self, vault: &str, proofs: &dyn Proofs) -> Result<(), ClientError> {
        let body = VaultBody { vault_id: Some(vault) };
        let begin: Begin = decode(&agent_call(
            self.transport,
            &self.config.token,
            "elevation-begin",
            &body,
        )?)?;
        let context = begin.vault_id.as_deref().unwrap_or(DEFAULT_VAULT);
        let message = self.message(context, &begin.request_id, &begin.challenge, true, false);
        let proof = proofs.elevation(&message)?;
        let enroll = EnrollBody {
            vault_id: vault.to_owned(),
            public_key: proof.public_key,
            signature: self.key.sign(&message)?,
            elevation_signature: proof.signature,
        };
        agent_call(self.transport, &self.config.token, "elevation-enroll", &enroll).map(|_| ())
    }

    /// Enrôle l'élévation sur chaque coffre éligible.
    ///
    /// Paramètres : les preuves (invite UAC par coffre enrôlé).
    /// Retour : la ligne `{"enrolled":<n>}` ; coffres ignorés si l'approbateur n'y est pas enregistré ou si
    /// l'élévation y est déjà active ; l'erreur du premier échec (annulation de l'UAC comprise) sinon.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn enroll_elevation(&self, proofs: &dyn Proofs) -> Result<String, ClientError> {
        let vaults: Vec<VaultEntry> = decode(&agent_call(
            self.transport,
            &self.config.token,
            "vaults",
            &VaultBody { vault_id: None },
        )?)?;
        let mut enrolled = 0;
        for vault in vaults {
            let body = VaultBody {
                vault_id: Some(&vault.id),
            };
            let status: Value = decode(&agent_call(self.transport, &self.config.token, "status", &body)?)?;
            let security = status.get("approvalSecurity").cloned().unwrap_or(Value::Null);
            if !flag(&security, "registered") || flag(&security, "elevationEnabled") {
                continue;
            }
            self.enroll_vault(&vault.id, proofs)?;
            enrolled += 1;
        }
        serde_json::to_string(&Enrolled { enrolled })
            .map_err(|_| ClientError::BadResponse("Output not encodable.".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approver_key::ApproverKey;
    use crate::config::ClientConfig;
    use crate::fake_transport::FakeTransport;
    use crate::proofs::FakeProofs;
    use crate::protector::ReversingProtector;
    use crate::server_address::ServerAddress;
    use crate::test_dir::{TestDir, sample_token};
    use crate::token::Token;
    use vaultagent_core::{ErrorCode, decision_message, verify_pss};

    const VAULTS: &str = "[{\"id\":\"a\"},{\"id\":\"b\"},{\"id\":\"c\"}]";
    const READY: &str = "{\"approvalSecurity\":{\"registered\":true,\"elevationEnabled\":false}}";
    const BEGIN: &str = "{\"vaultId\":\"a\",\"requestId\":\"enroll-elevation\",\"challenge\":\"chal\"}";

    /// Exécute l'enrôlement et rend le résultat avec la clé d'approbateur.
    fn run(fake: &FakeTransport, proofs: &FakeProofs) -> (Result<String, ClientError>, ApproverKey) {
        let dir = TestDir::new();
        let key = ApproverKey::load_or_create(&dir.path().join("k"), &ReversingProtector).unwrap();
        let config = ClientConfig {
            server: ServerAddress::parse("https://vault.example.test").unwrap(),
            token: Token::parse(&sample_token()).unwrap(),
        };
        let result = Approver {
            transport: fake,
            config: &config,
            key: &key,
        }
        .enroll_elevation(proofs);
        (result, key)
    }

    #[test]
    fn eligible_vault_is_enrolled_and_others_are_skipped() {
        let fake = FakeTransport::new()
            .reply(200, VAULTS)
            .reply(200, READY)
            .reply(200, BEGIN)
            .reply(200, "{\"elevationEnabled\":true,\"vaultId\":\"a\"}")
            .reply(200, "{\"approvalSecurity\":{\"registered\":false}}")
            .reply(
                200,
                "{\"approvalSecurity\":{\"registered\":true,\"elevationEnabled\":true}}",
            );
        let (result, key) = run(&fake, &FakeProofs::granting());
        assert_eq!(result.unwrap(), "{\"enrolled\":1}");
        let paths: Vec<String> = fake.calls().iter().map(|call| call.path.clone()).collect();
        assert_eq!(
            paths,
            [
                "vaults",
                "status",
                "elevation-begin",
                "elevation-enroll",
                "status",
                "status"
            ]
            .map(|route| format!("/api/agent/{route}"))
        );
        let body: Value = serde_json::from_str(&fake.calls()[3].body).unwrap();
        assert_eq!(body["vaultId"], "a");
        assert_eq!(body["publicKey"], "pub-elev");
        assert_eq!(body["elevationSignature"], "sig-elev");
        let message = decision_message(
            "https://vault.example.test",
            "a",
            "enroll-elevation",
            "chal",
            true,
            false,
        );
        verify_pss(
            &key.public_key().unwrap(),
            &message,
            body["signature"].as_str().unwrap(),
            ErrorCode::InvalidKey,
        )
        .unwrap();
    }

    #[test]
    fn nothing_to_enroll_prints_zero_without_uac() {
        let fake = FakeTransport::new().reply(200, "[]");
        let proofs = FakeProofs::refusing();
        assert_eq!(run(&fake, &proofs).0.unwrap(), "{\"enrolled\":0}");
        assert_eq!(proofs.asked.borrow().len(), 0);
    }

    #[test]
    fn cancelled_uac_stops_before_enrolling() {
        let fake = FakeTransport::new()
            .reply(200, VAULTS)
            .reply(200, READY)
            .reply(200, BEGIN);
        let error = run(&fake, &FakeProofs::refusing()).0.unwrap_err();
        assert_eq!(error.code(), "elevation-cancelled");
        assert_eq!(fake.calls().len(), 3);
    }

    #[test]
    fn server_refusal_is_relayed() {
        let fake = FakeTransport::new().reply(400, "{\"error\":\"approver-required\"}");
        assert_eq!(
            run(&fake, &FakeProofs::granting()).0.unwrap_err().code(),
            "approver-required"
        );
    }
}
