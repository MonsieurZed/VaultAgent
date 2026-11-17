//! Icône de la zone de notification : ajout, info-bulle, bulle d'information et suppression.

use std::ffi::c_void;

use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, Shell_NotifyIconW,
};

use crate::error::Win32Error;
use crate::native::hwnd;
use crate::ui_icon::Icon;
use crate::ui_text::{copy_into, last_error};
use crate::ui_window::{TRAY_CALLBACK, WindowRef};

/// Gravité de la bulle d'information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalloonKind {
    /// Information.
    Info,
    /// Avertissement.
    Warning,
}

/// Icône de barre attachée à une fenêtre ; retirée de la barre à la destruction.
#[derive(Debug)]
pub struct TrayIcon {
    window: WindowRef,
}

/// Prépare la structure commune d'une opération sur l'icône.
///
/// Paramètres : la fenêtre, l'icône et les drapeaux.
/// Retour : la structure remplie, le reste à zéro.
///
/// Sûreté : la structure est de type C simple ; la mettre à zéro est une valeur valide.
fn base(window: WindowRef, icon: isize, flags: u32) -> NOTIFYICONDATAW {
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = u32::try_from(size_of::<NOTIFYICONDATAW>()).unwrap_or(0);
    data.hWnd = hwnd(window.raw());
    data.uID = 1;
    data.uFlags = flags;
    data.uCallbackMessage = TRAY_CALLBACK;
    data.hIcon = icon as *mut c_void;
    data
}

/// Envoie une opération au shell.
///
/// Paramètres : le code d'opération et la structure.
/// Retour : `Ok` si le shell l'accepte, sinon l'erreur `Api`.
///
/// Sûreté : la structure est valide pendant l'appel et sa taille est annoncée dans `cbSize`.
fn notify(operation: u32, data: &NOTIFYICONDATAW) -> Result<(), Win32Error> {
    if unsafe { Shell_NotifyIconW(operation, std::ptr::from_ref(data)) } == 0 {
        return Err(last_error("Shell_NotifyIconW"));
    }
    Ok(())
}

impl TrayIcon {
    /// Ajoute l'icône à la zone de notification.
    ///
    /// Paramètres : la fenêtre qui reçoit les gestes, l'icône et l'info-bulle (127 unités au plus).
    /// Retour : l'icône de barre, ou l'erreur `Api` du shell.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn add(window: WindowRef, icon: &Icon, tip: &str) -> Result<Self, Win32Error> {
        let mut data = base(window, icon.raw(), NIF_MESSAGE | NIF_ICON | NIF_TIP);
        copy_into(&mut data.szTip, tip);
        notify(NIM_ADD, &data)?;
        Ok(Self { window })
    }

    /// Change l'info-bulle.
    ///
    /// Paramètres : le nouveau texte (127 unités au plus, tronqué au-delà).
    /// Retour : `Ok`, ou l'erreur `Api` du shell.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn set_tip(&self, tip: &str) -> Result<(), Win32Error> {
        let mut data = base(self.window, 0, NIF_TIP);
        copy_into(&mut data.szTip, tip);
        notify(NIM_MODIFY, &data)
    }

    /// Affiche une bulle d'information.
    ///
    /// Paramètres : le titre (63 unités), le texte (255 unités) et la gravité.
    /// Retour : `Ok`, ou l'erreur `Api` du shell.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn balloon(&self, title: &str, text: &str, kind: BalloonKind) -> Result<(), Win32Error> {
        let mut data = base(self.window, 0, NIF_INFO);
        copy_into(&mut data.szInfoTitle, title);
        copy_into(&mut data.szInfo, text);
        data.dwInfoFlags = match kind {
            BalloonKind::Info => NIIF_INFO,
            BalloonKind::Warning => NIIF_WARNING,
        };
        notify(NIM_MODIFY, &data)
    }
}

impl Drop for TrayIcon {
    /// Retire l'icône de la zone de notification.
    fn drop(&mut self) {
        let data = base(self.window, 0, 0);
        drop(notify(NIM_DELETE, &data));
    }
}
