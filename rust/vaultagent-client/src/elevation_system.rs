//! Opérations d'élévation du système (jeton, SID, dossier protégé, DPAPI machine, processus élevé), derrière un trait
//! que les tests remplacent par un faux système : aucune invite UAC dans les tests courants.

use std::path::Path;

use crate::error::ClientError;
use crate::protector::Protector;

/// Délai maximal de la validation Windows avant que le processus élevé soit tué.
pub const ELEVATION_TIMEOUT_SECONDS: u64 = 120;

/// Opérations du système dont la clé d'élévation et la signature élevée ont besoin.
pub trait ElevationSystem {
    /// Indique si le processus courant est élevé (administrateur).
    ///
    /// Retour : `true` pour un processus élevé.
    fn is_elevated(&self) -> bool;

    /// SID texte de l'utilisateur courant.
    ///
    /// Retour : le SID, ou `Config` si le système le refuse.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    fn current_sid(&self) -> Result<String, ClientError>;

    /// Crée un dossier réservé à SYSTEM et aux Administrateurs ; s'il existe, ne change rien.
    ///
    /// Paramètres : le chemin du dossier, dont le parent existe.
    ///
    /// # Errors
    ///
    /// `Config` si la création est refusée.
    fn create_private_directory(&self, path: &Path) -> Result<(), ClientError>;

    /// Protecteur DPAPI de portée machine pour la clé d'élévation.
    ///
    /// Retour : le protecteur, ou `Config` hors de Windows.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    fn machine_protector(&self) -> Result<Box<dyn Protector>, ClientError>;

    /// Relance le binaire courant avec élévation et attend sa fin.
    ///
    /// Paramètres : la ligne d'arguments du binaire.
    /// Retour : le code de sortie du processus élevé.
    ///
    /// # Errors
    ///
    /// `ElevationCancelled` si l'invite est annulée, `Elevation` si le lancement échoue ou expire.
    fn run_elevated(&self, args: &str) -> Result<u32, ClientError>;
}

/// Système d'élévation de la plateforme courante.
///
/// Retour : le système Windows.
#[cfg(windows)]
#[must_use]
pub fn system_elevation() -> Box<dyn ElevationSystem> {
    Box::new(crate::windows_elevation::WindowsElevation)
}

/// Système d'élévation de la plateforme courante.
///
/// Retour : un système qui n'est jamais élevé et refuse chaque opération.
#[cfg(not(windows))]
#[must_use]
pub fn system_elevation() -> Box<dyn ElevationSystem> {
    Box::new(Unsupported)
}

/// Système hors Windows : aucune élévation n'y existe.
#[cfg(not(windows))]
struct Unsupported;

/// Erreur d'usage commune au système non pris en charge.
#[cfg(not(windows))]
fn unsupported() -> ClientError {
    ClientError::Usage("The Windows validation is available on Windows only.".to_owned())
}

#[cfg(not(windows))]
impl ElevationSystem for Unsupported {
    /// Jamais élevé.
    fn is_elevated(&self) -> bool {
        false
    }

    /// Refuse hors de Windows.
    fn current_sid(&self) -> Result<String, ClientError> {
        Err(unsupported())
    }

    /// Refuse hors de Windows.
    fn create_private_directory(&self, _path: &Path) -> Result<(), ClientError> {
        Err(unsupported())
    }

    /// Refuse hors de Windows.
    fn machine_protector(&self) -> Result<Box<dyn Protector>, ClientError> {
        Err(unsupported())
    }

    /// Refuse hors de Windows.
    fn run_elevated(&self, _args: &str) -> Result<u32, ClientError> {
        Err(unsupported())
    }
}
