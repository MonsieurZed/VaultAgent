//! Preuves exigées par une décision d'approbation : validation Windows (UAC) et Windows Hello. Le trait permet aux
//! tests de remplacer les preuves ; la preuve Hello est signée par le système Hello puis vérifiée localement.

use std::path::Path;

use serde_json::Value;
use vaultagent_core::HelloCredential;

use crate::elevated_sign::elevated_sign;
use crate::elevation_sign::ElevatedProof;
use crate::elevation_system::ElevationSystem;
use crate::error::ClientError;
use crate::hello_proof::prove;
use crate::hello_system::HelloSystem;

/// Fournisseur des preuves d'une décision.
pub trait Proofs {
    /// Obtient la preuve d'élévation : invite UAC, signature par la clé d'élévation.
    ///
    /// Paramètres : le message de décision signé.
    /// Retour : la clé publique d'élévation et la signature, ou l'erreur de la validation (annulée, refusée).
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    fn elevation(&self, message: &str) -> Result<ElevatedProof, ClientError>;

    /// Obtient la preuve Windows Hello (`helloProof`).
    ///
    /// Paramètres : le identifiant de clé `helloCredential` de la demande et le message de décision.
    /// Retour : la preuve JSON, ou `HelloUnavailable`.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    fn hello(&self, credential: Option<&Value>, message: &str) -> Result<Value, ClientError>;
}

/// Preuves du poste : élévation par processus élevé, Hello par `webauthn.dll`.
pub struct SystemProofs<'a> {
    system: &'a dyn ElevationSystem,
    hello: &'a dyn HelloSystem,
    temp_dir: &'a Path,
}

impl<'a> SystemProofs<'a> {
    /// Crée les preuves du poste.
    ///
    /// Paramètres : le système d'élévation, le système Hello et le dossier temporaire de l'utilisateur.
    /// Retour : le fournisseur.
    #[must_use]
    pub fn new(system: &'a dyn ElevationSystem, hello: &'a dyn HelloSystem, temp_dir: &'a Path) -> Self {
        Self {
            system,
            hello,
            temp_dir,
        }
    }
}

impl Proofs for SystemProofs<'_> {
    /// Lance la signature élevée.
    fn elevation(&self, message: &str) -> Result<ElevatedProof, ClientError> {
        elevated_sign(self.system, self.temp_dir, message)
    }

    /// Signe le message avec Windows Hello et vérifie la preuve ; sans clé enrôlée, Hello est indisponible.
    fn hello(&self, credential: Option<&Value>, message: &str) -> Result<Value, ClientError> {
        let credential = credential
            .filter(|value| !value.is_null())
            .ok_or(ClientError::HelloUnavailable)?;
        let credential: HelloCredential = serde_json::from_value(credential.clone())
            .map_err(|_| ClientError::Hello("The Hello key of the request is invalid.".to_owned()))?;
        let body = prove(self.hello, &credential, message)?;
        serde_json::to_value(body).map_err(|_| ClientError::Hello("The Hello proof is not encodable.".to_owned()))
    }
}

/// Preuves factices des tests : réponses fixes, journal des messages signés.
pub struct FakeProofs {
    /// Signature d'élévation rendue, ou l'erreur à rendre.
    pub elevation: Result<ElevatedProof, ClientError>,
    /// Preuve Hello rendue, ou l'erreur à rendre.
    pub hello: Result<Value, ClientError>,
    /// Messages pour lesquels une preuve d'élévation a été demandée.
    pub asked: std::cell::RefCell<Vec<String>>,
}

impl FakeProofs {
    /// Preuves qui réussissent : élévation `pub-elev` / `sig-elev`, Hello `{"proof":"hello"}`.
    ///
    /// Retour : le fournisseur factice.
    #[must_use]
    pub fn granting() -> Self {
        Self {
            elevation: Ok(ElevatedProof {
                public_key: "pub-elev".to_owned(),
                signature: "sig-elev".to_owned(),
            }),
            hello: Ok(serde_json::json!({"version": 1, "proof": "hello"})),
            asked: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Preuves qui échouent toutes : annulation de l'UAC et Hello indisponible.
    ///
    /// Retour : le fournisseur factice.
    #[must_use]
    pub fn refusing() -> Self {
        Self {
            elevation: Err(ClientError::ElevationCancelled),
            hello: Err(ClientError::HelloUnavailable),
            asked: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl Proofs for FakeProofs {
    /// Note le message puis rend la réponse fixée.
    fn elevation(&self, message: &str) -> Result<ElevatedProof, ClientError> {
        self.asked.borrow_mut().push(message.to_owned());
        self.elevation.clone()
    }

    /// Rend la réponse fixée.
    fn hello(&self, _credential: Option<&Value>, _message: &str) -> Result<Value, ClientError> {
        self.hello.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_elevation::FakeElevation;
    use crate::fake_hello::FakeHello;

    #[test]
    fn system_proofs_refuse_hello_without_key_or_system() {
        let system = FakeElevation::elevated("S-1-5-21-1");
        let hello = FakeHello::unavailable();
        let proofs = SystemProofs::new(&system, &hello, Path::new("."));
        assert_eq!(proofs.hello(None, "m").unwrap_err().code(), "hello-unavailable");
        let null = Value::Null;
        assert_eq!(proofs.hello(Some(&null), "m").unwrap_err().code(), "hello-unavailable");
        let key = serde_json::json!({"id": "AA==", "keyType": "EC2", "first": "AA==", "second": "AA=="});
        assert_eq!(proofs.hello(Some(&key), "m").unwrap_err().code(), "hello-unavailable");
        let broken = serde_json::json!({"id": 3});
        assert_eq!(proofs.hello(Some(&broken), "m").unwrap_err().code(), "hello-failed");
        assert_eq!(hello.challenges().len(), 0);
    }

    #[test]
    fn fake_proofs_note_the_message() {
        let proofs = FakeProofs::granting();
        proofs.elevation("m1").unwrap();
        assert_eq!(proofs.asked.borrow().as_slice(), ["m1"]);
        assert_eq!(
            FakeProofs::refusing().elevation("m").unwrap_err().code(),
            "elevation-cancelled"
        );
    }
}
