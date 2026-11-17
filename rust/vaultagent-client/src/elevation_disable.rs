//! Retrait de la validation UAC (`elevation-begin {purpose:"disable"}` puis `elevation-disable`) et bascule entre
//! les modes carte et UAC pour l'application de barre.

use serde::Serialize;

use crate::api::{VaultBody, agent_call};
use crate::approval_request::DEFAULT_VAULT;
use crate::decide::Approver;
use crate::elevation_enroll::{VaultEntry, decode, flag};
use crate::error::ClientError;
use crate::proofs::Proofs;
use crate::tray_select::ValidationMode;

#[derive(Serialize)]
struct BeginDisable<'a> {
    #[serde(rename = "vaultId")]
    vault_id: &'a str,
    purpose: &'static str,
}

#[derive(serde::Deserialize)]
struct Context {
    #[serde(rename = "vaultId", default)]
    vault_id: Option<String>,
    #[serde(rename = "requestId")]
    request_id: String,
    challenge: String,
}

#[derive(Serialize)]
struct DisableBody<'a> {
    #[serde(rename = "vaultId")]
    vault_id: &'a str,
    signature: String,
    #[serde(rename = "elevationSignature")]
    elevation_signature: String,
}

/// Résultat d'un changement de mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Switched {
    /// Mode obtenu.
    pub mode: ValidationMode,
    /// Nombre de coffres modifiés ; 0 si rien n'était à changer.
    pub changed: usize,
}

impl Approver<'_> {
    /// Retire la clé d'élévation d'un coffre : contexte `disable`, signature élevée (invite UAC) puis retrait.
    fn disable_vault(&self, vault: &str, proofs: &dyn Proofs) -> Result<(), ClientError> {
        let begin = BeginDisable {
            vault_id: vault,
            purpose: "disable",
        };
        let context: Context = decode(&agent_call(
            self.transport,
            &self.config.token,
            "elevation-begin",
            &begin,
        )?)?;
        let id = context.vault_id.as_deref().unwrap_or(DEFAULT_VAULT);
        let message = self.message(id, &context.request_id, &context.challenge, true, false);
        let proof = proofs.elevation(&message)?;
        let body = DisableBody {
            vault_id: vault,
            signature: self.key.sign(&message)?,
            elevation_signature: proof.signature,
        };
        agent_call(self.transport, &self.config.token, "elevation-disable", &body).map(|_| ())
    }

    /// Retire la validation UAC des coffres où ce poste est approbateur et où elle est active.
    ///
    /// Paramètres : les preuves (l'invite UAC se fait par `proofs.elevation`).
    /// Retour : le nombre de coffres modifiés ; l'annulation de l'invite arrête tout et ne change rien de plus.
    ///
    /// # Errors
    ///
    /// Erreurs du serveur et du transport, `ElevationCancelled` si l'invite UAC est annulée.
    pub fn disable_elevation(&self, proofs: &dyn Proofs) -> Result<usize, ClientError> {
        let vaults: Vec<VaultEntry> = decode(&agent_call(
            self.transport,
            &self.config.token,
            "vaults",
            &VaultBody { vault_id: None },
        )?)?;
        let mut disabled = 0;
        for vault in vaults {
            let body = VaultBody {
                vault_id: Some(&vault.id),
            };
            let status: serde_json::Value = decode(&agent_call(self.transport, &self.config.token, "status", &body)?)?;
            let security = status.get("approvalSecurity").cloned().unwrap_or_default();
            if flag(&security, "registered") && flag(&security, "elevationEnabled") {
                self.disable_vault(&vault.id, proofs)?;
                disabled += 1;
            }
        }
        Ok(disabled)
    }

    /// Bascule la validation vers le mode demandé.
    ///
    /// Paramètres : le mode voulu et les preuves.
    /// Retour : le mode obtenu et le nombre de coffres modifiés ; `Uac` passe par `elevation-enroll`, `Card` par
    /// `disable_elevation`.
    ///
    /// # Errors
    ///
    /// Erreurs de `enroll_elevation` ou de `disable_elevation`.
    pub fn switch_validation(&self, target: ValidationMode, proofs: &dyn Proofs) -> Result<Switched, ClientError> {
        let changed = match target {
            ValidationMode::Uac => {
                let text = self.enroll_elevation(proofs)?;
                serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|value| value.get("enrolled").and_then(serde_json::Value::as_u64))
                    .and_then(|count| usize::try_from(count).ok())
                    .unwrap_or(0)
            }
            ValidationMode::Card => self.disable_elevation(proofs)?,
        };
        Ok(Switched { mode: target, changed })
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
    use vaultagent_core::{ErrorCode, decision_message, verify_pss};

    const ON: &str = "{\"approvalSecurity\":{\"registered\":true,\"elevationEnabled\":true}}";
    const OFF: &str = "{\"approvalSecurity\":{\"registered\":true,\"elevationEnabled\":false}}";
    const BEGIN: &str = "{\"vaultId\":\"a\",\"requestId\":\"disable-elevation\",\"challenge\":\"chal\"}";

    /// Lance le changement de mode avec un faux serveur.
    fn run(
        fake: &FakeTransport,
        proofs: &FakeProofs,
        target: ValidationMode,
    ) -> (Result<Switched, ClientError>, ApproverKey) {
        let dir = TestDir::new();
        let key = ApproverKey::load_or_create(&dir.path().join("k"), &ReversingProtector).unwrap();
        let config = ClientConfig {
            server: ServerAddress::parse("https://vault.example.test").unwrap(),
            token: Token::parse(&sample_token()).unwrap(),
        };
        let approver = Approver {
            transport: fake,
            config: &config,
            key: &key,
        };
        (approver.switch_validation(target, proofs), key)
    }

    #[test]
    fn card_mode_disables_only_vaults_with_elevation() {
        let fake = FakeTransport::new()
            .reply(200, "[{\"id\":\"a\"},{\"id\":\"b\"}]")
            .reply(200, ON)
            .reply(200, BEGIN)
            .reply(200, "{\"elevationEnabled\":false,\"vaultId\":\"a\"}")
            .reply(200, OFF);
        let proofs = FakeProofs::granting();
        let (result, key) = run(&fake, &proofs, ValidationMode::Card);
        assert_eq!(
            result.unwrap(),
            Switched {
                mode: ValidationMode::Card,
                changed: 1
            }
        );
        let calls = fake.calls();
        assert_eq!(calls[2].path, "/api/agent/elevation-begin");
        let begin: Value = serde_json::from_str(&calls[2].body).unwrap();
        assert_eq!(
            (begin["purpose"].as_str(), begin["vaultId"].as_str()),
            (Some("disable"), Some("a"))
        );
        let body: Value = serde_json::from_str(&calls[3].body).unwrap();
        assert_eq!(calls[3].path, "/api/agent/elevation-disable");
        assert_eq!(body["elevationSignature"], "sig-elev");
        let message = decision_message(
            "https://vault.example.test",
            "a",
            "disable-elevation",
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
        assert_eq!(proofs.asked.borrow().as_slice(), [message]);
    }

    #[test]
    fn cancelled_prompt_changes_nothing() {
        let fake = FakeTransport::new()
            .reply(200, "[{\"id\":\"a\"}]")
            .reply(200, ON)
            .reply(200, BEGIN);
        let (result, _) = run(&fake, &FakeProofs::refusing(), ValidationMode::Card);
        assert_eq!(result.unwrap_err().code(), "elevation-cancelled");
        assert_eq!(fake.calls().len(), 3);
    }

    #[test]
    fn nothing_to_disable_makes_no_prompt() {
        let fake = FakeTransport::new().reply(200, "[{\"id\":\"a\"}]").reply(200, OFF);
        let proofs = FakeProofs::refusing();
        let (result, _) = run(&fake, &proofs, ValidationMode::Card);
        assert_eq!(result.unwrap().changed, 0);
        assert!(proofs.asked.borrow().is_empty());
    }

    #[test]
    fn uac_mode_enrolls_through_the_existing_flow() {
        let fake = FakeTransport::new()
            .reply(200, "[{\"id\":\"a\"}]")
            .reply(200, OFF)
            .reply(200, BEGIN)
            .reply(200, "{\"elevationEnabled\":true,\"vaultId\":\"a\"}");
        let (result, _) = run(&fake, &FakeProofs::granting(), ValidationMode::Uac);
        assert_eq!(
            result.unwrap(),
            Switched {
                mode: ValidationMode::Uac,
                changed: 1
            }
        );
        assert_eq!(fake.calls()[3].path, "/api/agent/elevation-enroll");
    }
}
