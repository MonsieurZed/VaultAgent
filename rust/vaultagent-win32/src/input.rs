//! Frappe Unicode par `SendInput`, un caractère à la fois, avec contrôle de la cible avant chaque caractère.

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
};
use zeroize::Zeroizing;

use crate::error::Win32Error;

/// Nombre maximal d'événements pour un caractère : deux unités UTF-16, appui et relâchement chacune.
const MAX_EVENTS: usize = 4;

/// Échec d'une frappe : refus du contrôle de cible, ou erreur système.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeError<E> {
    /// Le contrôle de cible a refusé ; les caractères tapés avant sont comptés dans `typed`.
    Refused {
        /// Raison rendue par le contrôle.
        reason: E,
        /// Nombre de caractères déjà envoyés.
        typed: usize,
    },
    /// `SendInput` a échoué.
    Win32(Win32Error),
}

/// Construit l'événement clavier Unicode d'une unité UTF-16.
///
/// Paramètres : l'unité et `true` pour le relâchement.
/// Retour : l'événement `INPUT`.
fn unicode_event(unit: u16, release: bool) -> INPUT {
    let flags = if release {
        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
    } else {
        KEYEVENTF_UNICODE
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Envoie des événements d'entrée au système.
///
/// Paramètres : les événements.
/// Retour : le nombre d'événements acceptés par Windows.
///
/// Sûreté : le pointeur et le nombre viennent de la même tranche, valide pendant l'appel ; la taille est celle de `INPUT`.
fn send_events(events: &[INPUT]) -> u32 {
    let count = u32::try_from(events.len()).unwrap_or(0);
    let size = i32::try_from(size_of::<INPUT>()).unwrap_or(0);
    unsafe { SendInput(count, events.as_ptr(), size) }
}

/// Lit le code de la dernière erreur Win32 du fil.
///
/// Sûreté : `GetLastError` n'a aucun paramètre ni précondition.
fn last_error() -> u32 {
    unsafe { GetLastError() }
}

/// Tape un caractère : appui puis relâchement de chacune de ses unités UTF-16.
///
/// Paramètres : le caractère.
/// Retour : rien si Windows a accepté tous les événements.
///
/// # Errors
///
/// `Api { call: "SendInput", .. }` si Windows en refuse un (entrée bloquée par un processus plus privilégié).
fn send_char(character: char) -> Result<(), Win32Error> {
    let mut units = [0u16; 2];
    let encoded = character.encode_utf16(&mut units);
    let mut events = [unicode_event(0, false); MAX_EVENTS];
    let mut count = 0;
    for &unit in encoded.iter() {
        events[count] = unicode_event(unit, false);
        events[count + 1] = unicode_event(unit, true);
        count += 2;
    }
    let sent = send_events(&events[..count]);
    let cleared = [unicode_event(0, false); MAX_EVENTS];
    events.copy_from_slice(&cleared);
    units.fill(0);
    if usize::try_from(sent).is_ok_and(|accepted| accepted == count) {
        Ok(())
    } else {
        Err(Win32Error::Api {
            call: "SendInput",
            code: last_error(),
        })
    }
}

/// Tape un texte caractère par caractère, après un contrôle de la cible avant chacun.
///
/// Paramètres : le texte secret (jamais copié ni journalisé), le contrôle appelé avant chaque caractère.
/// Retour : le nombre de caractères tapés.
///
/// # Errors
///
/// `Refused` dès que le contrôle refuse : plus aucun événement n'est envoyé ; `Win32` si `SendInput` échoue.
pub fn type_text<E>(text: &Zeroizing<String>, mut check: impl FnMut() -> Result<(), E>) -> Result<usize, TypeError<E>> {
    let mut typed = 0;
    for character in text.chars() {
        check().map_err(|reason| TypeError::Refused { reason, typed })?;
        send_char(character).map_err(TypeError::Win32)?;
        typed += 1;
    }
    Ok(typed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusal_before_the_first_character_sends_nothing() {
        let text = Zeroizing::new("dummy-value".to_owned());
        let result = type_text(&text, || Err("not the test window"));
        assert_eq!(
            result,
            Err(TypeError::Refused {
                reason: "not the test window",
                typed: 0
            })
        );
    }

    #[test]
    fn empty_text_types_nothing_and_never_checks() {
        let text = Zeroizing::new(String::new());
        let result: Result<usize, TypeError<()>> = type_text(&text, || Err(()));
        assert_eq!(result, Ok(0));
    }
}
