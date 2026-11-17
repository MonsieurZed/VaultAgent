//! Mise au premier plan d'une fenêtre, par la méthode du fil attaché qui contourne la règle du vol de focus.

use std::thread::sleep;
use std::time::Duration;

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOUSEEVENTF_MOVE, mouse_event};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SW_RESTORE, SetForegroundWindow,
    ShowWindow,
};

use crate::error::Win32Error;
use crate::native::hwnd;

/// Délai laissé à Windows pour appliquer le changement de fenêtre active.
const SETTLE: Duration = Duration::from_millis(150);

/// Restaure la fenêtre si elle est réduite.
///
/// Paramètres : l'identifiant de fenêtre.
///
/// Sûreté : `IsIconic` et `ShowWindow` acceptent tout `HWND` et échouent sans effet s'il est invalide.
fn restore_if_minimized(handle: isize) {
    if unsafe { IsIconic(hwnd(handle)) } != 0 {
        unsafe {
            ShowWindow(hwnd(handle), SW_RESTORE);
        }
    }
}

/// Envoie un mouvement de souris nul, qui rend le fil appelant éligible au changement de premier plan.
///
/// Sûreté : `mouse_event` n'a pas de pointeur ; un déplacement relatif de 0 ne bouge pas le curseur.
fn nudge_input() {
    unsafe {
        mouse_event(MOUSEEVENTF_MOVE, 0, 0, 0, 0);
    }
}

/// Lit le fil de la fenêtre actuellement au premier plan.
///
/// Retour : l'identifiant de ce fil, ou 0 s'il n'y a pas de fenêtre active.
///
/// Sûreté : le pointeur de pid est nul, ce que `GetWindowThreadProcessId` accepte.
fn foreground_thread() -> u32 {
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut()) }
}

/// Lit le fil appelant.
///
/// Sûreté : `GetCurrentThreadId` n'a aucune précondition.
fn current_thread() -> u32 {
    unsafe { GetCurrentThreadId() }
}

/// Attache ou détache l'entrée du fil appelant à celle d'un autre fil.
///
/// Paramètres : le fil appelant, l'autre fil, vrai pour attacher.
/// Retour : vrai si Windows a exécuté la demande.
///
/// Sûreté : `AttachThreadInput` n'a pas de pointeur ; une paire de fils invalide est refusée par Windows.
fn attach(current: u32, other: u32, join: bool) -> bool {
    unsafe { AttachThreadInput(current, other, i32::from(join)) != 0 }
}

/// Demande à Windows de mettre la fenêtre au premier plan.
///
/// Paramètres : l'identifiant de fenêtre.
/// Retour : le code d'erreur Win32 si `SetForegroundWindow` est refusé.
///
/// Sûreté : `SetForegroundWindow` et `BringWindowToTop` acceptent tout `HWND`.
fn request_foreground(handle: isize) -> Result<(), Win32Error> {
    let accepted = unsafe { SetForegroundWindow(hwnd(handle)) } != 0;
    unsafe {
        BringWindowToTop(hwnd(handle));
    }
    if accepted {
        Ok(())
    } else {
        Err(Win32Error::Api {
            call: "SetForegroundWindow",
            code: unsafe { GetLastError() },
        })
    }
}

/// Met une fenêtre au premier plan : restauration, mouvement nul, entrée attachée, demande, attente.
///
/// Paramètres : l'identifiant de la fenêtre à activer.
/// Retour : rien si Windows a accepté la demande.
///
/// # Errors
///
/// `Api { call: "SetForegroundWindow", .. }` si Windows refuse ; l'appelant relit alors le premier plan,
/// seule preuve qui compte.
pub fn bring_to_front(handle: isize) -> Result<(), Win32Error> {
    restore_if_minimized(handle);
    nudge_input();
    let (current, other) = (current_thread(), foreground_thread());
    let attached = other != 0 && other != current && attach(current, other, true);
    let result = request_foreground(handle);
    if attached {
        attach(current, other, false);
    }
    sleep(SETTLE);
    result
}
