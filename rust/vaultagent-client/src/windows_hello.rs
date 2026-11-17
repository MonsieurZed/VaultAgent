//! Système Windows Hello réel : adapte `vaultagent-win32` au trait `HelloSystem`.

use vaultagent_core::digest;
use vaultagent_win32::{Win32Error, hello_available, hello_register, hello_sign, hello_window};

use crate::error::ClientError;
use crate::hello_system::{HelloAssertion, HelloRegistration, HelloStatus, HelloSystem};

/// Nom d'utilisateur affiché dans l'invite Windows Hello.
const USER_NAME: &str = "VaultAgent";

/// Windows Hello du poste courant.
pub struct WindowsHello;

/// Convertit un échec Win32 en erreur du client sans détail secret.
///
/// Paramètres : l'échec.
/// Retour : `HelloCancelled` pour l'annulation, `Hello` sinon.
fn failure(error: &Win32Error) -> ClientError {
    match error {
        Win32Error::Cancelled => ClientError::HelloCancelled,
        Win32Error::TimedOut => ClientError::Hello("The Windows Hello prompt timed out.".to_owned()),
        other => ClientError::Hello(other.to_string()),
    }
}

impl HelloSystem for WindowsHello {
    /// Interroge `webauthn.dll`, sans invite.
    fn status(&self) -> HelloStatus {
        let availability = hello_available();
        HelloStatus {
            available: availability.available,
            api_version: availability.api_version,
        }
    }

    /// Crée le justificatif ; l'identifiant d'utilisateur est l'empreinte du SID de l'utilisateur courant.
    fn register(&self, rp: &str, origin: &str) -> Result<HelloRegistration, ClientError> {
        let sid = vaultagent_win32::current_user_sid()
            .map_err(|_| ClientError::Config("Cannot read the current user identity.".to_owned()))?;
        let user_id = digest(sid.as_bytes());
        hello_register(hello_window(), user_id.as_bytes(), USER_NAME, rp, origin)
            .map(|created| HelloRegistration {
                credential_id: created.credential_id,
                authenticator_data: created.authenticator_data,
            })
            .map_err(|error| failure(&error))
    }

    /// Signe le défi ; la fenêtre parente est la fenêtre au premier plan.
    fn sign(
        &self,
        credential_id: &[u8],
        challenge: &str,
        rp: &str,
        origin: &str,
    ) -> Result<HelloAssertion, ClientError> {
        hello_sign(hello_window(), credential_id, challenge, rp, origin)
            .map(|proof| HelloAssertion {
                credential_id: proof.credential_id,
                client_data: proof.client_data,
                authenticator_data: proof.authenticator_data,
                signature: proof.signature,
            })
            .map_err(|error| failure(&error))
    }
}
