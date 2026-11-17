//! Contrôles enfants (texte, boutons, case à cocher, zone de saisie) en police Segoe UI, et actions sur les fenêtres.

use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Graphics::Gdi::CreateFontW;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CreateWindowExW, ES_AUTOHSCROLL,
    GetWindowTextLengthW, GetWindowTextW, SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE, SendMessageW, SetWindowPos,
    SetWindowTextW, ShowWindow, WM_SETFONT, WS_BORDER, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
};

use crate::error::Win32Error;
use crate::native::hwnd;
use crate::ui_text::{from_wide, last_error, wide};
use crate::ui_window::WindowRef;

const SS_NOPREFIX: u32 = 0x80;
const BST_CHECKED: usize = 1;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const FONT_HEIGHT: i32 = -16;
const FONT_WEIGHT: i32 = 400;
const DEFAULT_CHARSET: u32 = 1;

thread_local! {
    static FONT: Cell<isize> = const { Cell::new(0) };
}

/// Genre de contrôle enfant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// Texte fixe.
    Label,
    /// Bouton ordinaire.
    Button,
    /// Bouton par défaut (touche Entrée).
    DefaultButton,
    /// Case à cocher.
    Checkbox,
    /// Zone de saisie d'une ligne.
    Edit,
}

impl Control {
    /// Classe système et styles propres au genre.
    ///
    /// Retour : le nom de classe et les styles `WS_*`, `BS_*`, `ES_*`.
    fn parts(self) -> (&'static str, u32) {
        let base = WS_CHILD | WS_VISIBLE;
        match self {
            Self::Label => ("STATIC", base | SS_NOPREFIX),
            Self::Button => ("BUTTON", base | WS_TABSTOP | BS_PUSHBUTTON as u32),
            Self::DefaultButton => ("BUTTON", base | WS_TABSTOP | BS_DEFPUSHBUTTON as u32),
            Self::Checkbox => ("BUTTON", base | WS_TABSTOP | BS_AUTOCHECKBOX as u32),
            Self::Edit => ("EDIT", base | WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32),
        }
    }
}

/// Police Segoe UI partagée du fil, créée à la demande.
///
/// Retour : l'identifiant de la police, ou 0 si elle ne peut être créée (police système par défaut).
///
/// Sûreté : le nom de police UTF-16 vit pendant l'appel ; la police créée est gardée pour la durée du fil.
fn font() -> isize {
    FONT.with(|cell| {
        if cell.get() == 0 {
            let face = wide("Segoe UI");
            let created = unsafe {
                CreateFontW(
                    FONT_HEIGHT,
                    0,
                    0,
                    0,
                    FONT_WEIGHT,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET,
                    0,
                    0,
                    5,
                    0,
                    face.as_ptr(),
                )
            };
            cell.set(created as isize);
        }
        cell.get()
    })
}

impl WindowRef {
    /// Crée un contrôle enfant.
    ///
    /// Paramètres : le genre, le texte, l'identifiant de commande et le rectangle `(x, y, largeur, hauteur)`.
    /// Retour : le contrôle, ou l'erreur `Api` de la création.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    ///
    /// Sûreté : les chaînes UTF-16 vivent pendant l'appel ; l'identifiant de commande passe dans le paramètre de
    /// menu des fenêtres enfants, comme l'exige Win32 ; la police est un identifiant GDI valide ou nul.
    pub fn add_control(
        self,
        kind: Control,
        text: &str,
        id: u16,
        rect: (i32, i32, i32, i32),
    ) -> Result<WindowRef, Win32Error> {
        let (class, style) = kind.parts();
        let (class, text) = (wide(class), wide(text));
        let (x, y, width, height) = rect;
        let created = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                text.as_ptr(),
                style,
                x,
                y,
                width,
                height,
                hwnd(self.0),
                usize::from(id) as *mut c_void,
                GetModuleHandleW(null()),
                null(),
            )
        };
        if created.is_null() {
            return Err(last_error("CreateWindowExW"));
        }
        unsafe {
            SendMessageW(created, WM_SETFONT, font().cast_unsigned(), 1);
        }
        Ok(WindowRef(created as isize))
    }

    /// Remplace le texte de la fenêtre ou du contrôle.
    ///
    /// Paramètres : le nouveau texte.
    ///
    /// Sûreté : la chaîne UTF-16 vit pendant l'appel ; un `HWND` invalide échoue sans effet.
    pub fn set_text(self, text: &str) {
        let text = wide(text);
        unsafe {
            SetWindowTextW(hwnd(self.0), text.as_ptr());
        }
    }

    /// Lit le texte de la fenêtre ou du contrôle.
    ///
    /// Retour : le texte, vide si la fenêtre n'existe plus.
    ///
    /// Sûreté : la capacité annoncée est celle du tampon, qui vit pendant l'appel.
    #[must_use]
    pub fn text(self) -> String {
        let length = usize::try_from(unsafe { GetWindowTextLengthW(hwnd(self.0)) }).unwrap_or(0);
        let mut buffer = vec![0_u16; length + 1];
        let capacity = i32::try_from(buffer.len()).unwrap_or(0);
        unsafe {
            GetWindowTextW(hwnd(self.0), buffer.as_mut_ptr(), capacity);
        }
        from_wide(&buffer)
    }

    /// Active ou désactive le contrôle.
    ///
    /// Paramètres : vrai pour activer.
    ///
    /// Sûreté : un `HWND` invalide échoue sans effet.
    pub fn set_enabled(self, enabled: bool) {
        unsafe {
            EnableWindow(hwnd(self.0), i32::from(enabled));
        }
    }

    /// Coche ou décoche une case.
    ///
    /// Paramètres : vrai pour cocher.
    ///
    /// Sûreté : `SendMessageW` avec `BM_SETCHECK` n'utilise aucun pointeur.
    pub fn set_checked(self, checked: bool) {
        unsafe {
            SendMessageW(hwnd(self.0), BM_SETCHECK, usize::from(checked), 0);
        }
    }

    /// Lit l'état d'une case.
    ///
    /// Retour : vrai si cochée.
    ///
    /// Sûreté : `SendMessageW` avec `BM_GETCHECK` n'utilise aucun pointeur.
    #[must_use]
    pub fn is_checked(self) -> bool {
        unsafe { SendMessageW(hwnd(self.0), BM_GETCHECK, 0, 0).cast_unsigned() == BST_CHECKED }
    }

    /// Affiche la fenêtre.
    ///
    /// Paramètres : `activate` faux pour l'afficher sans lui donner le focus.
    ///
    /// Sûreté : un `HWND` invalide échoue sans effet.
    pub fn show(self, activate: bool) {
        let mode = if activate { SW_SHOW } else { SW_SHOWNOACTIVATE };
        unsafe {
            ShowWindow(hwnd(self.0), mode);
        }
    }

    /// Cache la fenêtre.
    ///
    /// Sûreté : un `HWND` invalide échoue sans effet.
    pub fn hide(self) {
        unsafe {
            ShowWindow(hwnd(self.0), SW_HIDE);
        }
    }

    /// Déplace la fenêtre sans changer sa taille ni son ordre, sans l'activer.
    ///
    /// Paramètres : la nouvelle position.
    ///
    /// Sûreté : un `HWND` invalide échoue sans effet.
    pub fn move_to(self, x: i32, y: i32) {
        unsafe {
            SetWindowPos(
                hwnd(self.0),
                null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}
