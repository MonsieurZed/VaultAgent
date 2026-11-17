//! Fenêtres de l'interface : classe Win32, procédure de fenêtre qui transmet des événements sûrs à un gestionnaire,
//! boucle de messages, minuteries et messages postés depuis d'autres fils.

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::rc::Rc;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, IDC_ARROW, LoadCursorW,
    RegisterClassExW, SetWindowLongPtrW, WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_CREATE, WM_DESTROY,
    WM_LBUTTONDBLCLK, WM_NCCREATE, WM_NCDESTROY, WM_RBUTTONUP, WM_TIMER, WNDCLASSEXW, WS_CAPTION, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPED, WS_POPUP, WS_SYSMENU,
};

use crate::error::Win32Error;
use crate::ui_text::{last_error, wide};

/// Message de rappel de l'icône de barre.
pub(crate) const TRAY_CALLBACK: u32 = WM_APP + 1;

/// Message posté depuis un autre fil ; `WPARAM` porte le code.
pub(crate) const POSTED: u32 = WM_APP + 2;

const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;

/// Geste sur l'icône de barre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// Clic droit : ouvrir le menu.
    Menu,
    /// Double clic : ouvrir la fenêtre principale.
    Open,
}

/// Événement rendu au gestionnaire d'une fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    /// La fenêtre vient d'être créée : le moment de créer les contrôles enfants.
    Created,
    /// Commande d'un contrôle enfant (bouton, case).
    Command(u16),
    /// Une minuterie de la fenêtre a sonné.
    Timer(usize),
    /// Un message posté par un autre fil, avec son code.
    Posted(usize),
    /// Geste sur l'icône de barre.
    Tray(TrayAction),
    /// L'utilisateur demande la fermeture.
    CloseRequested,
    /// La fenêtre est détruite.
    Destroyed,
}

/// Réaction à un événement : un gestionnaire par fenêtre.
pub trait UiHandler {
    /// Traite un événement.
    ///
    /// Paramètres : la fenêtre concernée et l'événement.
    /// Retour : aucun.
    fn handle(&mut self, window: WindowRef, event: UiEvent);
}

/// Identifiant copiable d'une fenêtre ; tout appel sur une fenêtre détruite échoue sans effet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRef(pub(crate) isize);

/// Poste des messages vers une fenêtre depuis n'importe quel fil.
#[derive(Debug, Clone, Copy)]
pub struct Poster(pub(crate) isize);

/// Genre de fenêtre de premier niveau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    /// Fenêtre cachée qui reçoit les messages de l'icône de barre.
    Hidden,
    /// Carte : au-dessus des autres fenêtres, sans focus ni bouton de barre des tâches.
    Card,
    /// Fenêtre de dialogue titrée, avec bouton de fermeture.
    Dialog,
}

impl WindowKind {
    /// Styles `WS_*` et `WS_EX_*` du genre.
    ///
    /// Retour : le couple (style, style étendu).
    fn styles(self) -> (u32, u32) {
        match self {
            Self::Hidden => (WS_OVERLAPPED, 0),
            Self::Card => (
                WS_POPUP | WS_CAPTION | WS_SYSMENU,
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            ),
            Self::Dialog => (WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU, 0),
        }
    }
}

/// Paramètres de création d'une fenêtre de premier niveau.
#[derive(Debug, Clone)]
pub struct WindowSpec<'a> {
    /// Nom de la classe de fenêtre.
    pub class: &'a str,
    /// Titre.
    pub title: &'a str,
    /// Genre de fenêtre.
    pub kind: WindowKind,
    /// Position, largeur et hauteur : `(x, y, largeur, hauteur)`.
    pub rect: (i32, i32, i32, i32),
}

type Slot = RefCell<Box<dyn UiHandler>>;

/// Donne le gestionnaire d'une fenêtre sous forme de référence comptée locale.
///
/// Paramètres : la fenêtre.
/// Retour : le gestionnaire, ou `None` avant `WM_NCCREATE` et après `WM_NCDESTROY`.
///
/// Sûreté : la valeur de `GWLP_USERDATA` est soit 0, soit un pointeur issu de `Rc::into_raw` posé par ce module et
/// libéré une seule fois à `WM_NCDESTROY` ; le compteur est incrémenté avant la reconstruction de la référence.
fn slot_of(window: HWND) -> Option<Rc<Slot>> {
    let raw = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const Slot;
    if raw.is_null() {
        return None;
    }
    unsafe {
        Rc::increment_strong_count(raw);
        Some(Rc::from_raw(raw))
    }
}

/// Transmet un événement au gestionnaire de la fenêtre, sauf si celui-ci est déjà en cours d'exécution.
///
/// Paramètres : la fenêtre et l'événement.
/// Retour : aucun.
fn dispatch(window: HWND, event: UiEvent) {
    if let Some(slot) = slot_of(window)
        && let Ok(mut handler) = slot.try_borrow_mut()
    {
        handler.handle(WindowRef(window as isize), event);
    }
}

/// Libère la référence du gestionnaire tenue par la fenêtre.
///
/// Paramètres : la fenêtre en cours de destruction.
/// Retour : aucun.
///
/// Sûreté : le pointeur est celui posé à `WM_NCCREATE` ; il est remis à zéro avant d'être rendu à `Rc`, donc libéré
/// une seule fois.
fn release_slot(window: HWND) {
    let raw = unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, 0) } as *const Slot;
    if !raw.is_null() {
        drop(unsafe { Rc::from_raw(raw) });
    }
}

/// Procédure commune des fenêtres de l'interface.
///
/// Sûreté : appelée par le système avec une fenêtre valide ; `lparam` de `WM_NCCREATE` pointe sur un
/// `CREATESTRUCTW` valide dont `lpCreateParams` vient de `create_window`.
unsafe extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_NCCREATE => {
            let create = lparam as *const CREATESTRUCTW;
            let raw = unsafe { (*create).lpCreateParams } as isize;
            unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, raw) };
        }
        WM_CREATE => dispatch(window, UiEvent::Created),
        WM_COMMAND => dispatch(window, UiEvent::Command(u16::try_from(wparam & 0xFFFF).unwrap_or(0))),
        WM_TIMER => dispatch(window, UiEvent::Timer(wparam)),
        POSTED => dispatch(window, UiEvent::Posted(wparam)),
        TRAY_CALLBACK => match u32::try_from(lparam & 0xFFFF).unwrap_or(0) {
            WM_RBUTTONUP | WM_CONTEXTMENU => dispatch(window, UiEvent::Tray(TrayAction::Menu)),
            WM_LBUTTONDBLCLK => dispatch(window, UiEvent::Tray(TrayAction::Open)),
            _ => {}
        },
        WM_CLOSE => dispatch(window, UiEvent::CloseRequested),
        WM_DESTROY => dispatch(window, UiEvent::Destroyed),
        WM_NCDESTROY => {
            let result = unsafe { DefWindowProcW(window, message, wparam, lparam) };
            release_slot(window);
            return result;
        }
        _ => {}
    }
    if message == WM_CLOSE {
        return 0;
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// Enregistre la classe de fenêtre si elle ne l'est pas déjà.
///
/// Paramètres : le nom de la classe.
/// Retour : `Ok` si la classe existe après l'appel, sinon l'erreur `Api`.
///
/// Sûreté : la structure de classe est locale, le nom UTF-16 vit pendant l'appel et la procédure est de ce module.
fn register(class: &str) -> Result<(), Win32Error> {
    let name = wide(class);
    let class_info = unsafe {
        WNDCLASSEXW {
            cbSize: u32::try_from(size_of::<WNDCLASSEXW>()).unwrap_or(0),
            style: 0,
            lpfnWndProc: Some(procedure),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(null()),
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: 16 as *mut c_void,
            lpszMenuName: null(),
            lpszClassName: name.as_ptr(),
            hIconSm: null_mut(),
        }
    };
    if unsafe { RegisterClassExW(&raw const class_info) } == 0 {
        let error = last_error("RegisterClassExW");
        return match error {
            Win32Error::Api { code, .. } if code == ERROR_CLASS_ALREADY_EXISTS => Ok(()),
            other => Err(other),
        };
    }
    Ok(())
}

/// Crée une fenêtre de premier niveau reliée à un gestionnaire.
///
/// Paramètres : les paramètres de création et le gestionnaire des événements.
/// Retour : la fenêtre ; l'événement `Created` est déjà passé ; erreur `Api` si la création échoue.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
///
/// Sûreté : les chaînes UTF-16 vivent pendant l'appel ; le paramètre de création est un pointeur `Rc::into_raw` dont
/// la procédure prend la propriété à `WM_NCCREATE` et la libère à `WM_NCDESTROY` ; si la création échoue avant
/// `WM_NCCREATE`, la référence reste perdue (fuite d'un gestionnaire, cas rare et sans danger).
pub fn create_window(spec: &WindowSpec<'_>, handler: Box<dyn UiHandler>) -> Result<WindowRef, Win32Error> {
    register(spec.class)?;
    let (class, title) = (wide(spec.class), wide(spec.title));
    let raw = Rc::into_raw(Rc::new(RefCell::new(handler)));
    let (x, y, width, height) = spec.rect;
    let (style, ex_style) = spec.kind.styles();
    let created = unsafe {
        CreateWindowExW(
            ex_style,
            class.as_ptr(),
            title.as_ptr(),
            style,
            x,
            y,
            width,
            height,
            null_mut(),
            null_mut(),
            GetModuleHandleW(null()),
            raw.cast::<c_void>(),
        )
    };
    if created.is_null() {
        return Err(last_error("CreateWindowExW"));
    }
    Ok(WindowRef(created as isize))
}
