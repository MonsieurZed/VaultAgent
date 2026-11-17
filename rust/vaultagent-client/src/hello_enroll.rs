//! Commandes `hello-status` et `hello-enroll` : disponibilité de Windows Hello, puis enrôlement du justificatif de ce
//! poste sur chaque coffre éligible (algorithme `EnrollHelloAsync` du client .NET).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use vaultagent_core::{HELLO_ORIGIN, HELLO_RP_ID, HelloCredential};

use crate::api::{VaultBody, agent_call};
use crate::approval_request::DEFAULT_VAULT;
use crate::decide::Approver;
use crate::elevation_enroll::{VaultEntry, decode, flag};
use crate::error::ClientError;
use crate::hello_cose::credential_from_registration;
use crate::hello_proof::{HelloProofBody, prove};
use crate::hello_system::HelloSystem;

/// Sortie de `hello-enroll` : le nombre de coffres enrôlés.
#[derive(Serialize)]
struct Enrolled {
    enrolled: usize,
}

/// Contexte d'enrôlement rendu par `hello-begin`.
#[derive(Deserialize)]
struct Begin {
    #[serde(rename = "vaultId", default)]
    vault_id: Option<String>,
    #[serde(rename = "requestId")]
    request_id: String,
    challenge: String,
    #[serde(rename = "helloCredential", default)]
    hello_credential: Option<HelloCredential>,
}

/// Corps de `hello-enroll`.
#[derive(Serialize)]
struct EnrollBody<'a> {
    #[serde(rename = "vaultId")]
    vault_id: &'a str,
    credential: &'a HelloCredential,
    #[serde(rename = "helloProof")]
    hello_proof: HelloProofBody,
    signature: String,
}

/// Sérialise une sortie de commande.
fn line(value: &impl Serialize) -> Result<String, ClientError> {
    serde_json::to_string(value).map_err(|_| ClientError::BadResponse("Output not encodable.".to_owned()))
}

/// Ligne de sortie de `hello-status`.
///
/// Paramètres : le système Hello.
/// Retour : `{"available":<bool>,"apiVersion":<n>}` ; aucune vérification n'est lancée.
///
/// # Errors
///
/// `BadResponse` si la sortie n'est pas encodable.
pub fn hello_status(system: &dyn HelloSystem) -> Result<String, ClientError> {
    line(&system.status())
}

/// Justificatif de ce poste : créé à la première demande (une invite), réutilisé ensuite.
fn local_credential(
    hello: &dyn HelloSystem,
    created: &mut Option<HelloCredential>,
) -> Result<HelloCredential, ClientError> {
    if let Some(existing) = created {
        return Ok(existing.clone());
    }
    let registration = hello.register(HELLO_RP_ID, HELLO_ORIGIN)?;
    let credential = credential_from_registration(&registration, HELLO_RP_ID)?;
    *created = Some(credential.clone());
    Ok(credential)
}

impl Approver<'_> {
    /// Ouvre le contexte d'enrôlement Hello d'un coffre.
    fn hello_begin(&self, vault: &str) -> Result<Begin, ClientError> {
        let body = VaultBody { vault_id: Some(vault) };
        decode(&agent_call(self.transport, &self.config.token, "hello-begin", &body)?)
    }

    /// Coffres dont l'approbateur est enregistré et où Hello n'est pas encore exigé de tous.
    fn hello_targets(&self, only: Option<&str>) -> Result<Vec<String>, ClientError> {
        let vaults: Vec<VaultEntry> = decode(&agent_call(
            self.transport,
            &self.config.token,
            "vaults",
            &VaultBody { vault_id: None },
        )?)?;
        let mut targets = Vec::new();
        for vault in vaults.into_iter().filter(|vault| only.is_none_or(|id| id == vault.id)) {
            let body = VaultBody {
                vault_id: Some(&vault.id),
            };
            let status: Value = decode(&agent_call(self.transport, &self.config.token, "status", &body)?)?;
            let security = status.get("approvalSecurity").cloned().unwrap_or(Value::Null);
            let done = flag(&security, "helloEnabled") && flag(&security, "helloRequiredForAll");
            if flag(&security, "registered") && !done {
                targets.push(vault.id);
            }
        }
        Ok(targets)
    }

    /// Enrôle Hello sur un coffre : contexte, signature Hello vérifiée, signature d'approbateur, enrôlement.
    fn enroll_hello_vault(
        &self,
        vault: &str,
        hello: &dyn HelloSystem,
        created: &mut Option<HelloCredential>,
    ) -> Result<(), ClientError> {
        let mut begin = self.hello_begin(vault)?;
        let credential = if let Some(existing) = begin.hello_credential.take() {
            existing
        } else {
            let local = local_credential(hello, created)?;
            begin = self.hello_begin(vault)?;
            local
        };
        let context = begin.vault_id.as_deref().unwrap_or(DEFAULT_VAULT);
        let message = self.message(context, &begin.request_id, &begin.challenge, true, false);
        let body = EnrollBody {
            vault_id: vault,
            credential: &credential,
            hello_proof: prove(hello, &credential, &message)?,
            signature: self.key.sign(&message)?,
        };
        agent_call(self.transport, &self.config.token, "hello-enroll", &body).map(|_| ())
    }

    /// Enrôle Windows Hello sur chaque coffre éligible.
    ///
    /// Paramètres : le système Hello et le coffre choisi par `--vault` (tous sinon).
    /// Retour : la ligne `{"enrolled":<n>}` ; zéro sans invite si aucun coffre n'est éligible ; le justificatif est
    /// créé une seule fois (une invite) puis chaque coffre demande une signature (une invite par coffre).
    ///
    /// # Errors
    ///
    /// `HelloUnavailable` sans Windows Hello, `HelloCancelled` ou `Hello` si une invite échoue ou si la preuve ne
    /// passe pas la vérification locale, sinon l'erreur classée du premier appel refusé.
    pub fn enroll_hello(&self, hello: &dyn HelloSystem, only: Option<&str>) -> Result<String, ClientError> {
        if !hello.status().available {
            return Err(ClientError::HelloUnavailable);
        }
        let mut created = None;
        let targets = self.hello_targets(only)?;
        for vault in &targets {
            self.enroll_hello_vault(vault, hello, &mut created)?;
        }
        line(&Enrolled {
            enrolled: targets.len(),
        })
    }
}
