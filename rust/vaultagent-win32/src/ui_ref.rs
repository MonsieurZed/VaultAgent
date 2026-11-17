//! Actions sur les fenêtres (destruction, minuteries, messages postés) et boucle de messages.

use std::ptr::null_mut;

use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, DispatchMessageW, GetAncestor, GetMessageW, IsDialogMessageW, KillTimer, MSG, PostMessageW,
    PostQuitMessage, SetTimer, TranslateMessage,
};

use crate::native::hwnd;
use crate::ui_window::{POSTED, Poster, WindowRef};

const GA_ROOT: u32 = 2;

impl WindowRef {
    /// Poste un message depuis ce fil ou un autre.
    ///
    /// Retour : un `Poster` copiable et transmissible entre fils.
    #[must_use]
    pub fn poster(self) -> Poster {
        Poster(self.0)
    }

    /// Détruit la fenêtre ; sans effet si elle n'existe plus.
    ///
    /// Sûreté : `DestroyWindow` accepte tout `HWND` et échoue sans effet s'il est invalide.
    pub fn destroy(self) {
        unsafe {
            DestroyWindow(hwnd(self.0));
        }
    }

    /// Démarre une minuterie répétée.
    ///
    /// Paramètres : l'identifiant de la minuterie et sa période en millisecondes.
    ///
    /// Sûreté : `SetTimer` accepte tout `HWND` et n'utilise pas de rappel (`None`).
    pub fn set_timer(self, id: usize, millis: u32) {
        unsafe {
            SetTimer(hwnd(self.0), id, millis, None);
        }
    }

    /// Arrête une minuterie.
    ///
    /// Paramètres : l'identifiant de la minuterie.
    ///
    /// Sûreté : `KillTimer` accepte tout `HWND` et échoue sans effet si la minuterie n'existe pas.
    pub fn kill_timer(self, id: usize) {
        unsafe {
            KillTimer(hwnd(self.0), id);
        }
    }

    /// Identifiant entier de la fenêtre.
    ///
    /// Retour : le `HWND` en entier.
    #[must_use]
    pub fn raw(self) -> isize {
        self.0
    }
}

impl Poster {
    /// Poste un code vers la fenêtre ; l'événement `Posted(code)` arrive sur le fil de la fenêtre.
    ///
    /// Paramètres : le code.
    /// Retour : aucun ; le message est ignoré si la fenêtre n'existe plus.
    ///
    /// Sûreté : `PostMessageW` est utilisable depuis tout fil avec tout `HWND`.
    pub fn post(self, code: usize) {
        unsafe {
            PostMessageW(hwnd(self.0), POSTED, code, 0);
        }
    }
}

/// Fait tourner la boucle de messages jusqu'à `quit`.
///
/// Retour : aucun ; la navigation au clavier des fenêtres de premier niveau est assurée.
///
/// Sûreté : `message` est une structure locale que `GetMessageW` remplit avant tout usage.
pub fn run_message_loop() {
    let mut message: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&raw mut message, null_mut(), 0, 0) } > 0 {
        let root = unsafe { GetAncestor(message.hwnd, GA_ROOT) };
        let handled = !root.is_null() && unsafe { IsDialogMessageW(root, &raw const message) } != 0;
        if !handled {
            unsafe {
                TranslateMessage(&raw const message);
                DispatchMessageW(&raw const message);
            }
        }
    }
}

/// Demande la fin de la boucle de messages du fil courant.
///
/// Sûreté : `PostQuitMessage` n'a aucune précondition.
pub fn quit() {
    unsafe { PostQuitMessage(0) }
}
