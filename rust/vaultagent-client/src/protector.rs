//! Protection des secrets stockés sur disque : trait injectable, et implémentation DPAPI (portée utilisateur courant,
//! entropie `VaultAgent.Remote.v1`) sous Windows, compatible avec le client .NET.

use zeroize::Zeroizing;

use crate::error::ClientError;

/// Entropie additionnelle DPAPI partagée avec le client .NET.
pub const ENTROPY: &[u8] = b"VaultAgent.Remote.v1";

/// Protège et déprotège des octets secrets.
pub trait Protector {
    /// Protège des octets.
    ///
    /// Paramètres : les octets en clair.
    /// Retour : les octets protégés, ou `Config` si la protection échoue.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    fn protect(&self, clear: &[u8]) -> Result<Vec<u8>, ClientError>;

    /// Déprotège des octets.
    ///
    /// Paramètres : les octets protégés.
    /// Retour : les octets en clair, effacés à la destruction, ou `Config` si les données sont illisibles.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    fn unprotect(&self, protected: &[u8]) -> Result<Zeroizing<Vec<u8>>, ClientError>;
}

/// DPAPI de l'utilisateur courant.
#[cfg(windows)]
pub struct DpapiProtector;

#[cfg(windows)]
impl Protector for DpapiProtector {
    /// Protège avec `CryptProtectData`, portée utilisateur courant.
    fn protect(&self, clear: &[u8]) -> Result<Vec<u8>, ClientError> {
        vaultagent_win32::protect(clear, ENTROPY, vaultagent_win32::Scope::CurrentUser)
            .map_err(|_| ClientError::Config("Cannot protect the secret with DPAPI.".to_owned()))
    }

    /// Déprotège avec `CryptUnprotectData`, portée utilisateur courant.
    fn unprotect(&self, protected: &[u8]) -> Result<Zeroizing<Vec<u8>>, ClientError> {
        vaultagent_win32::unprotect(protected, ENTROPY, vaultagent_win32::Scope::CurrentUser).map_err(|_| {
            ClientError::Config("Cannot read the DPAPI-protected secret (other user or damaged).".to_owned())
        })
    }
}

/// DPAPI de portée machine, utilisée par la clé d'élévation (lisible seulement via le dossier réservé aux
/// administrateurs).
#[cfg(windows)]
pub struct MachineDpapiProtector;

#[cfg(windows)]
impl Protector for MachineDpapiProtector {
    /// Protège avec `CryptProtectData`, portée machine.
    fn protect(&self, clear: &[u8]) -> Result<Vec<u8>, ClientError> {
        vaultagent_win32::protect(clear, ENTROPY, vaultagent_win32::Scope::LocalMachine)
            .map_err(|_| ClientError::Config("Cannot protect the secret with machine DPAPI.".to_owned()))
    }

    /// Déprotège avec `CryptUnprotectData`, portée machine.
    fn unprotect(&self, protected: &[u8]) -> Result<Zeroizing<Vec<u8>>, ClientError> {
        vaultagent_win32::unprotect(protected, ENTROPY, vaultagent_win32::Scope::LocalMachine)
            .map_err(|_| ClientError::Config("Cannot read the machine-protected secret (damaged).".to_owned()))
    }
}

/// Protecteur du système courant.
///
/// Retour : DPAPI sous Windows ; ailleurs `Config`, car le stockage protégé n'y existe pas.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
#[cfg(windows)]
pub fn system_protector() -> Result<Box<dyn Protector>, ClientError> {
    Ok(Box::new(DpapiProtector))
}

/// Protecteur du système courant.
///
/// Retour : DPAPI sous Windows ; ailleurs `Config`, car le stockage protégé n'y existe pas.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
#[cfg(not(windows))]
pub fn system_protector() -> Result<Box<dyn Protector>, ClientError> {
    Err(ClientError::Config(
        "Protected storage is only available on Windows; use --config <file>.".to_owned(),
    ))
}

/// Protecteur de test : inverse l'ordre des octets, sans aucune confidentialité.
pub struct ReversingProtector;

impl Protector for ReversingProtector {
    /// Inverse les octets.
    fn protect(&self, clear: &[u8]) -> Result<Vec<u8>, ClientError> {
        Ok(clear.iter().rev().copied().collect())
    }

    /// Inverse les octets.
    fn unprotect(&self, protected: &[u8]) -> Result<Zeroizing<Vec<u8>>, ClientError> {
        Ok(Zeroizing::new(protected.iter().rev().copied().collect()))
    }
}

#[cfg(test)]
#[cfg(windows)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_protector_round_trips_and_hides_the_value() {
        let protected = DpapiProtector.protect(b"valeur-factice").unwrap();
        assert!(!protected.windows(14).any(|part| part == b"valeur-factice"));
        assert_eq!(
            DpapiProtector.unprotect(&protected).unwrap().as_slice(),
            b"valeur-factice"
        );
    }

    #[test]
    fn machine_protector_round_trips_and_is_not_readable_as_user_data() {
        let protected = MachineDpapiProtector.protect(b"valeur-factice").unwrap();
        assert!(!protected.windows(14).any(|part| part == b"valeur-factice"));
        assert_eq!(
            MachineDpapiProtector.unprotect(&protected).unwrap().as_slice(),
            b"valeur-factice"
        );
        assert_eq!(MachineDpapiProtector.unprotect(b"abc").unwrap_err().code(), "config");
    }

    #[test]
    fn damaged_data_is_a_config_error() {
        assert_eq!(DpapiProtector.unprotect(b"abc").unwrap_err().code(), "config");
    }
}
