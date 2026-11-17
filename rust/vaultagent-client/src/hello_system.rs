//! Opérations Windows Hello du système (disponibilité, enregistrement, signature), derrière un trait que les tests
//! remplacent par un faux système : aucune invite Hello dans les tests courants.

use serde::Serialize;

use crate::error::ClientError;

/// Disponibilité de Windows Hello, telle que rendue par `hello-status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HelloStatus {
    /// Un authentificateur de plateforme avec vérification de l'utilisateur est utilisable.
    pub available: bool,
    /// Version de l'API WebAuthn de Windows (0 si absente).
    #[serde(rename = "apiVersion")]
    pub api_version: u32,
}

/// Justificatif créé par le système : identifiant et données de l'authentificateur (clé COSE comprise).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelloRegistration {
    /// Identifiant du justificatif.
    pub credential_id: Vec<u8>,
    /// Données de l'authentificateur de l'attestation.
    pub authenticator_data: Vec<u8>,
}

/// Éléments bruts d'une preuve WebAuthn `get`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelloAssertion {
    /// Identifiant du justificatif utilisé.
    pub credential_id: Vec<u8>,
    /// `clientData` JSON envoyé à l'authentificateur.
    pub client_data: Vec<u8>,
    /// Données de l'authentificateur signées.
    pub authenticator_data: Vec<u8>,
    /// Signature ECDSA (DER) ou RSA PKCS#1 v1.5.
    pub signature: Vec<u8>,
}

/// Opérations de Windows Hello dont l'enrôlement et la décision ont besoin.
pub trait HelloSystem {
    /// Interroge la disponibilité, sans invite.
    ///
    /// Retour : la disponibilité et la version de l'API.
    fn status(&self) -> HelloStatus;

    /// Crée le justificatif de ce poste (l'invite Hello apparaît).
    ///
    /// Paramètres : l'identifiant du site (RP) et l'origine du `clientData`.
    /// Retour : l'identifiant et les données de l'authentificateur.
    ///
    /// # Errors
    ///
    /// `HelloUnavailable` hors de Windows, `HelloCancelled` si l'utilisateur annule, `Hello` pour tout autre échec.
    fn register(&self, rp: &str, origin: &str) -> Result<HelloRegistration, ClientError>;

    /// Signe un défi avec le justificatif enrôlé (l'invite Hello apparaît).
    ///
    /// Paramètres : l'identifiant du justificatif, le défi `base64url`, le RP et l'origine.
    /// Retour : les éléments bruts de la preuve.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles de `register`.
    fn sign(
        &self,
        credential_id: &[u8],
        challenge: &str,
        rp: &str,
        origin: &str,
    ) -> Result<HelloAssertion, ClientError>;
}

/// Système Hello de la plateforme courante.
///
/// Retour : le système Windows.
#[cfg(windows)]
#[must_use]
pub fn system_hello() -> Box<dyn HelloSystem> {
    Box::new(crate::windows_hello::WindowsHello)
}

/// Système Hello de la plateforme courante.
///
/// Retour : un système sans Hello, qui refuse l'enregistrement et la signature.
#[cfg(not(windows))]
#[must_use]
pub fn system_hello() -> Box<dyn HelloSystem> {
    Box::new(Unsupported)
}

/// Système hors Windows : Windows Hello n'y existe pas.
#[cfg(not(windows))]
struct Unsupported;

#[cfg(not(windows))]
impl HelloSystem for Unsupported {
    /// Jamais disponible.
    fn status(&self) -> HelloStatus {
        HelloStatus {
            available: false,
            api_version: 0,
        }
    }

    /// Refuse hors de Windows.
    fn register(&self, _rp: &str, _origin: &str) -> Result<HelloRegistration, ClientError> {
        Err(ClientError::HelloUnavailable)
    }

    /// Refuse hors de Windows.
    fn sign(
        &self,
        _credential_id: &[u8],
        _challenge: &str,
        _rp: &str,
        _origin: &str,
    ) -> Result<HelloAssertion, ClientError> {
        Err(ClientError::HelloUnavailable)
    }
}
