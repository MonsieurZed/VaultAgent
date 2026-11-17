//! Actions de l'application de barre sur le serveur : validation en une étape, acceptation automatique, verrouillage.

use serde::Deserialize;

use crate::api::{VaultBody, agent_call};
use crate::approval_request::{ApprovalRequest, DEFAULT_VAULT};
use crate::decide::Approver;
use crate::elevation_enroll::{VaultEntry, decode};
use crate::error::ClientError;
use crate::proofs::Proofs;

/// Issue d'une décision en une étape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// La demande est approuvée.
    Approved,
    /// Un refus signé a été envoyé.
    Denied,
}

#[derive(Deserialize)]
struct Context {
    #[serde(rename = "vaultId", default)]
    vault_id: Option<String>,
    #[serde(rename = "requestId")]
    request_id: String,
    challenge: String,
}

impl Approver<'_> {
    /// Valide une demande en une étape (mode UAC) : approbation avec invite directe, refus signé si elle est annulée.
    ///
    /// Paramètres : la demande et les preuves (UAC et Hello).
    /// Retour : `Approved`, ou `Denied` si l'invite UAC ou Hello est annulée ou indisponible et que le refus signé
    /// est parti.
    ///
    /// # Errors
    ///
    /// Les erreurs du serveur et du transport, y compris celles du refus signé.
    pub fn decide_one_step(&self, request: &ApprovalRequest, proofs: &dyn Proofs) -> Result<Decision, ClientError> {
        match self.decide_unlock(request, true, false, proofs) {
            Ok(_) => Ok(Decision::Approved),
            Err(ClientError::ElevationCancelled | ClientError::HelloUnavailable | ClientError::HelloCancelled) => self
                .decide_unlock(request, false, false, proofs)
                .map(|_| Decision::Denied),
            Err(other) => Err(other),
        }
    }

    /// Désactive l'acceptation automatique d'un coffre : défi `approval-context` puis refus signé.
    ///
    /// Paramètres : le coffre et les preuves (non sollicitées, un refus n'en exige pas).
    /// Retour : `Ok` quand le refus est accepté par le serveur.
    ///
    /// # Errors
    ///
    /// Les erreurs du serveur et du transport.
    pub fn reset_automatic(&self, vault: &str, proofs: &dyn Proofs) -> Result<(), ClientError> {
        let body = VaultBody { vault_id: Some(vault) };
        let context: Context = decode(&agent_call(
            self.transport,
            &self.config.token,
            "approval-context",
            &body,
        )?)?;
        let request = ApprovalRequest {
            vault_id: Some(context.vault_id.unwrap_or_else(|| DEFAULT_VAULT.to_owned())),
            request_id: context.request_id,
            challenge: context.challenge,
            hello_required: false,
            elevation_required: false,
            hello_credential: None,
        };
        self.decide_unlock(&request, false, false, proofs).map(|_| ())
    }

    /// Désactive l'acceptation automatique de tous les coffres.
    ///
    /// Paramètres : les preuves.
    /// Retour : le nombre de coffres traités.
    ///
    /// # Errors
    ///
    /// La première erreur rencontrée.
    pub fn reset_all_automatic(&self, proofs: &dyn Proofs) -> Result<usize, ClientError> {
        let vaults = self.vault_ids()?;
        vaults
            .iter()
            .try_for_each(|vault| self.reset_automatic(vault, proofs))
            .map(|()| vaults.len())
    }

    /// Verrouille tous les coffres.
    ///
    /// Retour : le nombre de coffres verrouillés.
    ///
    /// # Errors
    ///
    /// La première erreur rencontrée.
    pub fn lock_all(&self) -> Result<usize, ClientError> {
        let vaults = self.vault_ids()?;
        vaults.iter().try_for_each(|vault| {
            agent_call(
                self.transport,
                &self.config.token,
                "lock",
                &VaultBody { vault_id: Some(vault) },
            )
            .map(|_| ())
        })?;
        Ok(vaults.len())
    }

    /// Liste les identifiants des coffres du jeton.
    fn vault_ids(&self) -> Result<Vec<String>, ClientError> {
        let list: Vec<VaultEntry> = decode(&agent_call(
            self.transport,
            &self.config.token,
            "vaults",
            &VaultBody { vault_id: None },
        )?)?;
        Ok(list.into_iter().map(|vault| vault.id).collect())
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
    use serde_json::Value;

    const ANSWER: &str = "{\"vaultId\":\"default\",\"requestId\":\"r1\",\"state\":\"approved\"}";

    /// Configuration, clé et dossier de test.
    fn setup() -> (ClientConfig, ApproverKey, TestDir) {
        let dir = TestDir::new();
        let key = ApproverKey::load_or_create(&dir.path().join("k"), &ReversingProtector).unwrap();
        let config = ClientConfig {
            server: ServerAddress::parse("https://vault.example.test").unwrap(),
            token: Token::parse(&sample_token()).unwrap(),
        };
        (config, key, dir)
    }

    /// Demande de test.
    fn request() -> ApprovalRequest {
        ApprovalRequest {
            vault_id: Some("a".to_owned()),
            request_id: "r1".to_owned(),
            challenge: "chal".to_owned(),
            hello_required: false,
            elevation_required: true,
            hello_credential: None,
        }
    }

    /// Lit le corps JSON de la requête numéro `index`.
    fn body(fake: &FakeTransport, index: usize) -> Value {
        serde_json::from_str(&fake.calls()[index].body).unwrap()
    }

    #[test]
    fn uac_mode_approves_in_one_step() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        let proofs = FakeProofs::granting();
        assert_eq!(
            approver.decide_one_step(&request(), &proofs).unwrap(),
            Decision::Approved
        );
        assert_eq!(proofs.asked.borrow().len(), 1);
        assert_eq!(body(&fake, 0)["approve"], true);
        assert_eq!(body(&fake, 0)["remember"], false);
    }

    #[test]
    fn cancelled_uac_sends_a_signed_denial() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        assert_eq!(
            approver.decide_one_step(&request(), &FakeProofs::refusing()).unwrap(),
            Decision::Denied
        );
        let sent = body(&fake, 0);
        assert_eq!(sent["approve"], false);
        assert!(sent.get("elevationSignature").is_none() && sent["signature"].is_string());
        assert_eq!(fake.calls().len(), 1);
    }

    #[test]
    fn server_failure_is_not_turned_into_a_denial() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(400, "{\"error\":\"request-expired\"}");
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        let error = approver
            .decide_one_step(&request(), &FakeProofs::granting())
            .unwrap_err();
        assert_eq!(error.code(), "request-expired");
        assert_eq!(fake.calls().len(), 1);
    }

    #[test]
    fn automatic_acceptance_is_reset_on_every_vault() {
        let (config, key, _dir) = setup();
        let context = "{\"vaultId\":\"a\",\"requestId\":\"reset-auto\",\"challenge\":\"c\"}";
        let fake = FakeTransport::new()
            .reply(200, "[{\"id\":\"a\"},{\"id\":\"b\"}]")
            .reply(200, context)
            .reply(200, ANSWER)
            .reply(
                200,
                "{\"vaultId\":\"b\",\"requestId\":\"reset-auto\",\"challenge\":\"d\"}",
            )
            .reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        let proofs = FakeProofs::refusing();
        assert_eq!(approver.reset_all_automatic(&proofs).unwrap(), 2);
        let calls = fake.calls();
        assert_eq!(calls[1].path, "/api/agent/approval-context");
        assert_eq!(calls[2].path, "/api/agent/decide-unlock");
        assert_eq!(body(&fake, 2)["requestId"], "reset-auto");
        assert_eq!(body(&fake, 2)["approve"], false);
        assert_eq!(body(&fake, 4)["vaultId"], "b");
        assert!(proofs.asked.borrow().is_empty());
    }

    #[test]
    fn lock_reaches_every_vault() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new()
            .reply(200, "[{\"id\":\"a\"},{\"id\":\"b\"}]")
            .reply(200, "{\"locked\":true}")
            .reply(200, "{\"locked\":true}");
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        assert_eq!(approver.lock_all().unwrap(), 2);
        assert_eq!(body(&fake, 2)["vaultId"], "b");
        assert_eq!(fake.calls()[1].path, "/api/agent/lock");
    }
}
