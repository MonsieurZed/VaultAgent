//! Création d'un dossier protégé : sans héritage, contrôle total pour SYSTEM et les Administrateurs seulement.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;

use crate::error::Win32Error;

/// Descripteur protégé : SYSTEM et Administrateurs en contrôle total, hérité par les fichiers et sous-dossiers.
const PRIVATE_SDDL: &str = "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";

/// Convertit un texte en UTF-16 terminé par un zéro.
fn wide(text: &std::ffi::OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

/// Crée le dossier avec le descripteur donné.
///
/// Paramètres : le chemin UTF-16 terminé par zéro et le descripteur de sécurité.
/// Retour : `Ok(())` si créé, `Ok(())` aussi s'il existait déjà (rien n'est changé), sinon l'erreur Windows.
///
/// Sûreté : `SECURITY_ATTRIBUTES` est une variable locale dont le descripteur vient de Windows et reste valide
/// pendant l'appel ; le chemin est terminé par un zéro.
fn create_with(path: &[u16], descriptor: *mut c_void) -> Result<(), Win32Error> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(std::mem::size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    if unsafe { CreateDirectoryW(path.as_ptr(), &raw const attributes) } != 0 {
        return Ok(());
    }
    let code = unsafe { GetLastError() };
    if code == ERROR_ALREADY_EXISTS {
        return Ok(());
    }
    Err(Win32Error::Api {
        call: "CreateDirectoryW",
        code,
    })
}

/// Crée un dossier réservé à SYSTEM et aux Administrateurs ; s'il existe déjà, ne change rien.
///
/// Paramètres : le chemin du dossier, dont le parent doit exister.
/// Retour : `Ok(())` à la création ou si le dossier existe.
///
/// Sûreté : le descripteur rendu par `ConvertStringSecurityDescriptorToSecurityDescriptorW` est libéré une seule
/// fois par `LocalFree` après l'appel de création, et ne sort jamais de cette fonction.
///
/// # Errors
///
/// `Win32Error::Api` si la conversion du descripteur ou la création est refusée.
pub fn create_private_directory(path: &Path) -> Result<(), Win32Error> {
    let sddl = wide(std::ffi::OsStr::new(PRIVATE_SDDL));
    let mut descriptor: *mut c_void = null_mut();
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &raw mut descriptor,
            null_mut(),
        )
    };
    if converted == 0 {
        return Err(Win32Error::Api {
            call: "ConvertStringSecurityDescriptorToSecurityDescriptorW",
            code: unsafe { GetLastError() },
        });
    }
    let result = create_with(&wide(path.as_os_str()), descriptor);
    unsafe { LocalFree(descriptor) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_directory_is_left_untouched() {
        let dir = std::env::temp_dir();
        create_private_directory(&dir).unwrap();
        assert!(dir.is_dir());
    }

    #[test]
    fn missing_parent_is_an_api_error() {
        let path = std::env::temp_dir()
            .join(format!("vaultagent-absent-{}", std::process::id()))
            .join("child");
        assert!(matches!(
            create_private_directory(&path),
            Err(Win32Error::Api {
                call: "CreateDirectoryW",
                ..
            })
        ));
    }
}
