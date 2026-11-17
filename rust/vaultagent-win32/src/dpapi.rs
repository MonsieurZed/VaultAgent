//! Enveloppes sûres de DPAPI (`CryptProtectData` et `CryptUnprotectData`) : protection d'un secret pour l'utilisateur
//! courant ou pour la machine, sans aucune interface utilisateur.

use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_LOCAL_MACHINE, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use zeroize::Zeroizing;

use crate::error::Win32Error;

/// Portée de la protection DPAPI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Lisible seulement par le compte Windows qui a protégé le secret.
    CurrentUser,
    /// Lisible par tout compte de cette machine.
    LocalMachine,
}

impl Scope {
    /// Drapeaux DPAPI de la portée, avec l'interdiction de toute interface utilisateur.
    fn flags(self) -> u32 {
        match self {
            Self::CurrentUser => CRYPTPROTECT_UI_FORBIDDEN,
            Self::LocalMachine => CRYPTPROTECT_UI_FORBIDDEN | CRYPTPROTECT_LOCAL_MACHINE,
        }
    }
}

/// Décrit un tampon d'octets pour DPAPI.
///
/// Paramètres : les octets, qui doivent rester valides pendant l'appel qui reçoit le bloc.
/// Retour : le bloc ; sa longueur est bornée à `u32::MAX`.
fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
        pbData: bytes.as_ptr().cast_mut(),
    }
}

/// Copie puis libère la mémoire rendue par Windows, après l'avoir effacée.
///
/// Paramètres : le bloc de sortie d'un appel DPAPI réussi.
/// Retour : la copie dans un `Zeroizing`.
///
/// Sûreté : après un appel réussi, `pbData` désigne `cbData` octets alloués par Windows pour l'appelant ; ils sont
/// lus, écrasés de zéros puis libérés une seule fois par `LocalFree`.
fn take_output(output: &CRYPT_INTEGER_BLOB) -> Zeroizing<Vec<u8>> {
    if output.pbData.is_null() {
        return Zeroizing::new(Vec::new());
    }
    let length = output.cbData as usize;
    let copy = Zeroizing::new(unsafe { std::slice::from_raw_parts(output.pbData, length) }.to_vec());
    unsafe {
        std::ptr::write_bytes(output.pbData, 0, length);
        LocalFree(output.pbData.cast());
    }
    copy
}

/// Protège un secret avec DPAPI.
///
/// Paramètres : les octets en clair, l'entropie additionnelle (même valeur à la lecture) et la portée.
/// Retour : les octets protégés.
///
/// Sûreté : les blocs d'entrée visent des tampons empruntés valides pendant l'appel, la sortie est une variable locale
/// et sa mémoire est reprise par `take_output`.
///
/// # Errors
///
/// `Win32Error::Api` avec `CryptProtectData` si Windows refuse.
pub fn protect(data: &[u8], entropy: &[u8], scope: Scope) -> Result<Vec<u8>, Win32Error> {
    let input = blob(data);
    let extra = blob(entropy);
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let entropy_ptr = if entropy.is_empty() { null() } else { &raw const extra };
    let done = unsafe {
        CryptProtectData(
            &raw const input,
            null(),
            entropy_ptr,
            null(),
            null(),
            scope.flags(),
            &raw mut output,
        )
    };
    if done == 0 {
        return Err(Win32Error::Api {
            call: "CryptProtectData",
            code: unsafe { GetLastError() },
        });
    }
    Ok(take_output(&output).to_vec())
}

/// Déprotège un secret DPAPI.
///
/// Paramètres : les octets protégés, l'entropie utilisée à la protection et la portée attendue.
/// Retour : les octets en clair, effacés à la destruction.
///
/// Sûreté : mêmes garanties que `protect` ; le pointeur de description reste nul, donc rien n'est à libérer.
///
/// # Errors
///
/// `Win32Error::Api` avec `CryptUnprotectData` si les données sont corrompues, d'une autre entropie ou d'un autre
/// utilisateur.
pub fn unprotect(data: &[u8], entropy: &[u8], scope: Scope) -> Result<Zeroizing<Vec<u8>>, Win32Error> {
    let input = blob(data);
    let extra = blob(entropy);
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let entropy_ptr = if entropy.is_empty() { null() } else { &raw const extra };
    let done = unsafe {
        CryptUnprotectData(
            &raw const input,
            null_mut(),
            entropy_ptr,
            null(),
            null(),
            scope.flags(),
            &raw mut output,
        )
    };
    if done == 0 {
        return Err(Win32Error::Api {
            call: "CryptUnprotectData",
            code: unsafe { GetLastError() },
        });
    }
    Ok(take_output(&output))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTROPY: &[u8] = b"VaultAgent.Test.v1";

    #[test]
    fn current_user_round_trip() {
        let protected = protect(b"valeur-factice", ENTROPY, Scope::CurrentUser).unwrap();
        assert_ne!(protected.as_slice(), b"valeur-factice");
        let clear = unprotect(&protected, ENTROPY, Scope::CurrentUser).unwrap();
        assert_eq!(clear.as_slice(), b"valeur-factice");
    }

    #[test]
    fn local_machine_round_trip() {
        let protected = protect(b"autre-valeur", ENTROPY, Scope::LocalMachine).unwrap();
        let clear = unprotect(&protected, ENTROPY, Scope::LocalMachine).unwrap();
        assert_eq!(clear.as_slice(), b"autre-valeur");
    }

    #[test]
    fn wrong_entropy_is_refused() {
        let protected = protect(b"valeur-factice", ENTROPY, Scope::CurrentUser).unwrap();
        let error = unprotect(&protected, b"autre", Scope::CurrentUser).unwrap_err();
        assert!(matches!(
            error,
            Win32Error::Api {
                call: "CryptUnprotectData",
                ..
            }
        ));
    }

    #[test]
    fn corrupted_data_is_refused() {
        let mut protected = protect(b"valeur-factice", ENTROPY, Scope::CurrentUser).unwrap();
        let middle = protected.len() / 2;
        protected[middle] ^= 0xff;
        assert!(unprotect(&protected, ENTROPY, Scope::CurrentUser).is_err());
        assert!(unprotect(b"pas-du-dpapi", ENTROPY, Scope::CurrentUser).is_err());
    }

    #[test]
    fn empty_secret_round_trips() {
        let protected = protect(b"", ENTROPY, Scope::CurrentUser).unwrap();
        assert!(unprotect(&protected, ENTROPY, Scope::CurrentUser).unwrap().is_empty());
    }
}
