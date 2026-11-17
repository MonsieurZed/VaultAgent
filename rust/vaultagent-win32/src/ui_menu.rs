//! Menu contextuel de l'icône de barre : entrées désactivables, séparateurs, choix rendu à l'appelant.

use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_GRAYED, MF_SEPARATOR, MF_STRING, PostMessageW,
    SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
};

use crate::error::Win32Error;
use crate::native::hwnd;
use crate::ui_text::{last_error, wide};
use crate::ui_window::WindowRef;

/// Entrée d'un menu contextuel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    /// Entrée avec identifiant (non nul), texte et état.
    Item {
        /// Identifiant rendu quand l'entrée est choisie.
        id: u16,
        /// Texte affiché.
        text: String,
        /// Faux pour griser l'entrée.
        enabled: bool,
    },
    /// Trait de séparation.
    Separator,
}

/// Affiche le menu à la position du curseur et attend le choix.
///
/// Paramètres : la fenêtre propriétaire et les entrées.
/// Retour : l'identifiant choisi, `None` si le menu est refermé sans choix ; erreur `Api` si le menu ne peut être
/// construit.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
///
/// Sûreté : les textes UTF-16 vivent pendant chaque `AppendMenuW`, qui les copie ; le menu est détruit avant le
/// retour ; le curseur est lu dans une structure locale.
pub fn show_menu(owner: WindowRef, entries: &[MenuEntry]) -> Result<Option<u16>, Win32Error> {
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return Err(last_error("CreatePopupMenu"));
    }
    for entry in entries {
        match entry {
            MenuEntry::Item { id, text, enabled } => {
                let flags = if *enabled { MF_STRING } else { MF_STRING | MF_GRAYED };
                let text = wide(text);
                unsafe { AppendMenuW(menu, flags, usize::from(*id), text.as_ptr()) };
            }
            MenuEntry::Separator => {
                unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null()) };
            }
        }
    }
    let mut cursor = POINT { x: 0, y: 0 };
    let chosen = unsafe {
        GetCursorPos(&raw mut cursor);
        SetForegroundWindow(hwnd(owner.raw()));
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            0,
            hwnd(owner.raw()),
            std::ptr::null(),
        );
        PostMessageW(hwnd(owner.raw()), WM_NULL, 0, 0);
        DestroyMenu(menu);
        chosen
    };
    Ok(u16::try_from(chosen).ok().filter(|id| *id != 0))
}
