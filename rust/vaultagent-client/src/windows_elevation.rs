//! Système d'élévation Windows : adapte `vaultagent-win32` au trait `ElevationSystem`.

use std::path::Path;
use std::time::Duration;

use vaultagent_win32::Win32Error;

use crate::elevation_system::{ELEVATION_TIMEOUT_SECONDS, ElevationSystem};
use crate::error::ClientError;
use crate::protector::{MachineDpapiProtector, Protector};

/// Élévation du poste Windows courant.
pub struct WindowsElevation;

/// Convertit un échec Win32 en erreur du client sans détail secret.
///
/// Paramètres : l'échec.
/// Retour : `ElevationCancelled` pour l'annulation de l'invite, `Elevation` sinon.
fn failure(error: &Win32Error) -> ClientError {
    match error {
        Win32Error::Cancelled => ClientError::ElevationCancelled,
        Win32Error::TimedOut => ClientError::Elevation("The Windows validation timed out.".to_owned()),
        other => ClientError::Elevation(other.to_string()),
    }
}

impl ElevationSystem for WindowsElevation {
    /// Interroge le jeton du processus.
    fn is_elevated(&self) -> bool {
        vaultagent_win32::is_elevated()
    }

    /// Lit le SID du jeton courant.
    fn current_sid(&self) -> Result<String, ClientError> {
        vaultagent_win32::current_user_sid()
            .map_err(|_| ClientError::Config("Cannot read the current user identity.".to_owned()))
    }

    /// Crée le dossier protégé par descripteur SDDL.
    fn create_private_directory(&self, path: &Path) -> Result<(), ClientError> {
        vaultagent_win32::create_private_directory(path)
            .map_err(|_| ClientError::Config("Cannot create the protected elevation folder.".to_owned()))
    }

    /// DPAPI de portée machine.
    fn machine_protector(&self) -> Result<Box<dyn Protector>, ClientError> {
        Ok(Box::new(MachineDpapiProtector))
    }

    /// Relance l'exécutable courant avec le verbe `runas`.
    fn run_elevated(&self, args: &str) -> Result<u32, ClientError> {
        let exe = std::env::current_exe()
            .map_err(|_| ClientError::Elevation("Cannot find the current executable.".to_owned()))?;
        vaultagent_win32::run_elevated(&exe, args, Duration::from_secs(ELEVATION_TIMEOUT_SECONDS))
            .map_err(|error| failure(&error))
    }
}
