//! Identité du processus courant : appartenance aux Administrateurs (élévation) et SID texte de l'utilisateur.

use std::ptr::{null_mut, slice_from_raw_parts};

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{
    CheckTokenMembership, CreateWellKnownSid, GetTokenInformation, SECURITY_MAX_SID_SIZE, TOKEN_QUERY, TOKEN_USER,
    TokenUser, WinBuiltinAdministratorsSid,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::error::Win32Error;

/// Erreur d'appel système avec le dernier code Windows.
///
/// Sûreté : `GetLastError` n'a ni paramètre ni précondition.
fn api_error(call: &'static str) -> Win32Error {
    Win32Error::Api {
        call,
        code: unsafe { GetLastError() },
    }
}

/// Indique si le jeton du processus appartient aux Administrateurs avec le groupe activé (processus élevé).
///
/// Retour : `true` pour un processus élevé, `false` pour un jeton filtré ; `false` aussi si Windows refuse de
/// répondre.
///
/// Sûreté : le SID est écrit dans un tampon local de taille maximale d'un SID, puis lu par `CheckTokenMembership`
/// avec un jeton nul (jeton effectif du fil) et un booléen de sortie local.
#[must_use]
pub fn is_elevated() -> bool {
    let mut sid = [0u8; SECURITY_MAX_SID_SIZE as usize];
    let mut size = SECURITY_MAX_SID_SIZE;
    let created = unsafe {
        CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            null_mut(),
            sid.as_mut_ptr().cast(),
            &raw mut size,
        )
    };
    if created == 0 {
        return false;
    }
    let mut member = 0;
    let checked = unsafe { CheckTokenMembership(null_mut(), sid.as_mut_ptr().cast(), &raw mut member) };
    checked != 0 && member != 0
}

/// Ouvre le jeton du processus courant en lecture.
///
/// Retour : la poignée du jeton, à fermer par `CloseHandle`.
///
/// Sûreté : le pseudo-handle du processus courant est toujours valide ; la poignée de sortie est une variable locale.
fn open_token() -> Result<HANDLE, Win32Error> {
    let mut token: HANDLE = null_mut();
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) };
    if opened == 0 {
        return Err(api_error("OpenProcessToken"));
    }
    Ok(token)
}

/// Lit le `TOKEN_USER` d'un jeton dans un tampon aligné.
///
/// Paramètres : la poignée du jeton.
/// Retour : le tampon, aligné sur 8 octets, qui contient le `TOKEN_USER` et son SID.
///
/// Sûreté : la première requête rend la taille nécessaire, la seconde remplit un tampon de cette taille.
fn token_user_buffer(token: HANDLE) -> Result<Vec<u64>, Win32Error> {
    let mut needed = 0u32;
    unsafe { GetTokenInformation(token, TokenUser, null_mut(), 0, &raw mut needed) };
    if needed == 0 {
        return Err(api_error("GetTokenInformation"));
    }
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    let filled = unsafe { GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), needed, &raw mut needed) };
    if filled == 0 {
        return Err(api_error("GetTokenInformation"));
    }
    Ok(buffer)
}

/// Convertit le SID d'un `TOKEN_USER` en texte.
///
/// Paramètres : le tampon rempli par `token_user_buffer`.
/// Retour : le SID texte, par exemple `S-1-5-21-…`.
///
/// Sûreté : le tampon contient un `TOKEN_USER` valide ; la chaîne rendue par Windows est copiée puis libérée une
/// seule fois par `LocalFree`.
fn sid_text(buffer: &[u64]) -> Result<String, Win32Error> {
    let user = buffer.as_ptr().cast::<TOKEN_USER>();
    let mut text: *mut u16 = null_mut();
    let converted = unsafe { ConvertSidToStringSidW((*user).User.Sid, &raw mut text) };
    if converted == 0 {
        return Err(api_error("ConvertSidToStringSidW"));
    }
    let mut length = 0usize;
    while unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    let value = String::from_utf16_lossy(unsafe { &*slice_from_raw_parts(text, length) });
    unsafe { LocalFree(text.cast()) };
    Ok(value)
}

/// SID texte de l'utilisateur du processus courant (le même que `WindowsIdentity.GetCurrent().User`).
///
/// Retour : le SID, ou `Win32Error::Api` si Windows refuse une des lectures.
///
/// Sûreté : la poignée du jeton vient de `open_token` et est fermée une seule fois par `CloseHandle`.
///
/// # Errors
///
/// `Win32Error::Api` avec le nom de l'appel refusé.
pub fn current_user_sid() -> Result<String, Win32Error> {
    let token = open_token()?;
    let result = token_user_buffer(token).and_then(|buffer| sid_text(&buffer));
    unsafe { CloseHandle(token) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_sid_has_the_sid_shape() {
        let sid = current_user_sid().unwrap();
        assert!(sid.starts_with("S-1-5-"), "{sid}");
        assert!(
            sid.bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'-' || byte == b'S')
        );
        assert_eq!(current_user_sid().unwrap(), sid);
    }

    #[test]
    fn elevation_query_answers_without_prompt() {
        assert_eq!(is_elevated(), is_elevated());
    }
}
