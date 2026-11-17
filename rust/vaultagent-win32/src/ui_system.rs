//! Services système de l'interface : instance unique, zone de travail, ouverture d'une adresse, préférence de langue.

use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, RECT};
use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETWORKAREA, SW_SHOWNORMAL, SystemParametersInfoW};

use crate::error::Win32Error;
use crate::ui_text::{from_wide, last_error, wide};

const ERROR_ALREADY_EXISTS: u32 = 183;
const SHELL_SUCCESS_ABOVE: isize = 32;

/// Verrou d'instance unique : un mutex nommé tenu tant que l'objet vit.
#[derive(Debug)]
pub struct SingleInstance(isize);

impl SingleInstance {
    /// Prend le verrou de l'instance.
    ///
    /// Paramètres : le nom du mutex (propre à la session de l'utilisateur).
    /// Retour : le verrou, ou `None` si une autre instance le tient déjà ; erreur `Api` si le mutex est refusé.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    ///
    /// Sûreté : le nom UTF-16 vit pendant l'appel ; `GetLastError` est lu aussitôt après `CreateMutexW`.
    pub fn acquire(name: &str) -> Result<Option<Self>, Win32Error> {
        let name = wide(name);
        let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(last_error("CreateMutexW"));
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe { CloseHandle(handle) };
            return Ok(None);
        }
        Ok(Some(Self(handle as isize)))
    }
}

impl Drop for SingleInstance {
    /// Libère le mutex.
    ///
    /// Sûreté : la poignée vient de `CreateMutexW` et n'est fermée qu'ici.
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0 as HANDLE) };
    }
}

/// Zone de travail de l'écran principal.
///
/// Retour : `(gauche, haut, droite, bas)` en pixels, ou l'erreur `Api`.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
///
/// Sûreté : le pointeur de sortie vise un `RECT` local valide pendant l'appel.
pub fn work_area() -> Result<(i32, i32, i32, i32), Win32Error> {
    let mut area = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&raw mut area).cast(), 0) } == 0 {
        return Err(last_error("SystemParametersInfoW"));
    }
    Ok((area.left, area.top, area.right, area.bottom))
}

/// Ouvre une adresse avec l'application par défaut.
///
/// Paramètres : l'adresse (HTTPS).
/// Retour : `Ok`, ou l'erreur `Api` si le shell refuse.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
///
/// Sûreté : toutes les chaînes UTF-16 vivent pendant l'appel ; les paramètres facultatifs sont nuls.
pub fn open_url(url: &str) -> Result<(), Win32Error> {
    let (verb, url) = (wide("open"), wide(url));
    let result = unsafe { ShellExecuteW(null_mut(), verb.as_ptr(), url.as_ptr(), null(), null(), SW_SHOWNORMAL) };
    if (result as isize) <= SHELL_SUCCESS_ABOVE {
        return Err(Win32Error::Api {
            call: "ShellExecuteW",
            code: u32::try_from(result as isize).unwrap_or(0),
        });
    }
    Ok(())
}

/// Lit une valeur texte de `HKEY_CURRENT_USER`.
///
/// Paramètres : la sous-clé et le nom de la valeur.
/// Retour : le texte, ou `None` si la valeur est absente ou illisible.
///
/// Sûreté : le tampon de sortie et sa taille en octets viennent du même vecteur local.
#[must_use]
pub fn registry_text(subkey: &str, value: &str) -> Option<String> {
    let (subkey, value) = (wide(subkey), wide(value));
    let mut buffer = vec![0_u16; 128];
    let mut size = u32::try_from(buffer.len() * 2).unwrap_or(0);
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &raw mut size,
        )
    };
    (status == 0).then(|| from_wide(&buffer))
}

/// Nom de la langue de Windows pour l'utilisateur.
///
/// Retour : un nom de langue comme `fr-FR`, vide si Windows ne le donne pas.
///
/// Sûreté : la capacité annoncée est celle du tampon local.
#[must_use]
pub fn user_locale_name() -> String {
    let mut buffer = vec![0_u16; 85];
    let capacity = i32::try_from(buffer.len()).unwrap_or(0);
    let written = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), capacity) };
    if written <= 0 {
        return String::new();
    }
    from_wide(&buffer)
}
