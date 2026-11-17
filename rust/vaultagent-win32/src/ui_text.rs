//! Aides communes de l'interface Win32 : texte UTF-16 et erreur du dernier appel système.

use windows_sys::Win32::Foundation::GetLastError;

use crate::error::Win32Error;

/// Encode un texte en UTF-16 terminé par un zéro.
///
/// Paramètres : le texte.
/// Retour : les unités UTF-16 suivies d'un zéro.
pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Décode un tampon UTF-16 jusqu'au premier zéro.
///
/// Paramètres : le tampon.
/// Retour : le texte, avec remplacement des unités invalides.
pub(crate) fn from_wide(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

/// Copie un texte dans un tampon fixe, tronqué et terminé par un zéro.
///
/// Paramètres : le tampon de destination et le texte.
/// Retour : aucun ; le tampon reçoit au plus sa taille moins un unités, puis un zéro.
pub(crate) fn copy_into(buffer: &mut [u16], text: &str) {
    let capacity = buffer.len().saturating_sub(1);
    let mut count = 0;
    for (slot, unit) in buffer.iter_mut().zip(text.encode_utf16().take(capacity)) {
        *slot = unit;
        count += 1;
    }
    if let Some(end) = buffer.get_mut(count) {
        *end = 0;
    }
}

/// Construit l'erreur du dernier appel système.
///
/// Paramètres : le nom de la fonction appelée.
/// Retour : l'erreur `Api` avec le code de `GetLastError`.
///
/// Sûreté : `GetLastError` n'a aucun paramètre ni précondition.
pub(crate) fn last_error(call: &'static str) -> Win32Error {
    Win32Error::Api {
        call,
        code: unsafe { GetLastError() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_round_trips_accents() {
        let text = "Coffre déverrouillé";
        assert_eq!(from_wide(&wide(text)), text);
        assert_eq!(wide("a").last(), Some(&0));
    }

    #[test]
    fn copy_into_truncates_and_terminates() {
        let mut buffer = [7_u16; 4];
        copy_into(&mut buffer, "abcdef");
        assert_eq!(from_wide(&buffer), "abc");
        assert_eq!(buffer[3], 0);
        let mut empty: [u16; 0] = [];
        copy_into(&mut empty, "x");
    }
}
