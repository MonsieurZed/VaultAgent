//! Fenêtre factice des tests Win32 : une fenêtre de premier niveau titrée avec un contrôle EDIT enfant.

use std::ptr::null_mut;
use std::thread::sleep;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, GetWindowTextW, MSG, PM_REMOVE, PeekMessageW,
    TranslateMessage, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

use crate::native::hwnd;

/// Fenêtre factice avec son champ de saisie.
pub struct TestWindow {
    /// Identifiant de la fenêtre de premier niveau.
    pub handle: isize,
    edit: isize,
}

/// Encode un texte en UTF-16 terminé par un zéro.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Crée une fenêtre du système.
///
/// Paramètres : la classe système, le titre, les styles, la position et la taille, le parent.
/// Retour : l'identifiant de la fenêtre, ou 0 en cas d'échec.
///
/// Sûreté : les deux chaînes sont terminées par zéro et vivent jusqu'à la fin de l'appel ; la classe est une
/// classe système, donc sans instance ni paramètre de création.
fn create(class: &str, title: &str, style: u32, rect: (i32, i32, i32, i32), parent: isize) -> isize {
    let (class, title) = (wide(class), wide(title));
    unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            style,
            rect.0,
            rect.1,
            rect.2,
            rect.3,
            hwnd(parent),
            null_mut(),
            null_mut(),
            std::ptr::null(),
        ) as isize
    }
}

/// Donne le focus clavier à une fenêtre du fil courant.
///
/// Paramètres : l'identifiant de la fenêtre.
///
/// Sûreté : `SetFocus` accepte tout `HWND` du fil appelant et échoue sans effet sinon.
fn focus(handle: isize) {
    unsafe {
        SetFocus(hwnd(handle));
    }
}

/// Traite les messages en attente du fil courant.
///
/// Sûreté : `message` est une structure locale initialisée, valide pour la durée de chaque appel.
fn drain_messages() {
    let mut message = MSG {
        hwnd: null_mut(),
        message: 0,
        wParam: 0,
        lParam: 0,
        time: 0,
        pt: POINT { x: 0, y: 0 },
    };
    while unsafe { PeekMessageW(&raw mut message, null_mut(), 0, 0, PM_REMOVE) } != 0 {
        unsafe {
            TranslateMessage(&raw const message);
            DispatchMessageW(&raw const message);
        }
    }
}

/// Lit le texte d'un contrôle du fil courant.
///
/// Paramètres : l'identifiant du contrôle.
/// Retour : son contenu (au plus 256 unités).
///
/// Sûreté : le tampon local et sa capacité correspondent ; l'appel est synchrone.
fn read_text(handle: isize) -> String {
    let mut buffer = [0u16; 256];
    let copied = unsafe { GetWindowTextW(hwnd(handle), buffer.as_mut_ptr(), 256) };
    String::from_utf16_lossy(&buffer[..usize::try_from(copied).unwrap_or(0)])
}

/// Traite les messages du fil courant pendant une durée, pour que le champ reçoive les frappes en attente.
///
/// Paramètres : la durée.
pub fn pump(duration: Duration) {
    let end = Instant::now() + duration;
    while Instant::now() < end {
        drain_messages();
        sleep(Duration::from_millis(10));
    }
}

impl TestWindow {
    /// Ouvre la fenêtre factice.
    ///
    /// Paramètres : le titre exact de la fenêtre.
    /// Retour : la fenêtre, ou `None` si Windows refuse de la créer.
    pub fn open(title: &str) -> Option<Self> {
        let style = WS_OVERLAPPEDWINDOW | WS_VISIBLE;
        let handle = create("STATIC", title, style, (120, 120, 520, 160), 0);
        if handle == 0 {
            return None;
        }
        let edit_style = WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL as u32;
        let edit = create("EDIT", "", edit_style, (10, 10, 480, 30), handle);
        (edit != 0).then_some(Self { handle, edit })
    }

    /// Donne le focus clavier au champ de saisie.
    pub fn focus_edit(&self) {
        focus(self.edit);
    }

    /// Contenu actuel du champ de saisie.
    pub fn text(&self) -> String {
        read_text(self.edit)
    }
}

impl Drop for TestWindow {
    /// Détruit la fenêtre.
    ///
    /// Sûreté : la fenêtre appartient au fil courant et n'est plus utilisée ensuite.
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(hwnd(self.handle));
        }
    }
}
