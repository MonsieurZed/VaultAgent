//! Identité des fenêtres : fenêtre au premier plan et fenêtres visibles de premier niveau, sans `unsafe` public.

use crate::error::Win32Error;
use crate::native::{foreground_handle, process_image, visible_handles, window_process_id, window_title};

/// Instantané de l'identité d'une fenêtre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    /// Identifiant de la fenêtre (`HWND`).
    pub handle: isize,
    /// Identifiant du processus propriétaire.
    pub process_id: u32,
    /// Nom du processus sans dossier ni `.exe`.
    pub process: String,
    /// Titre de la fenêtre au moment de la lecture.
    pub title: String,
}

/// Extrait le nom d'un processus d'un chemin d'image : dernier segment, sans `.exe`.
///
/// Paramètres : le chemin complet de l'image.
/// Retour : le nom, par exemple `chrome` pour `C:\Program Files\Chrome\chrome.exe`.
#[must_use]
pub fn process_name_from_path(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    let stem_length = file.len().saturating_sub(4);
    match file.get(stem_length..) {
        Some(tail) if file.len() > 4 && tail.eq_ignore_ascii_case(".exe") => file[..stem_length].to_owned(),
        _ => file.to_owned(),
    }
}

/// Lit l'identité d'une fenêtre.
///
/// Paramètres : l'identifiant de fenêtre.
/// Retour : l'instantané, ou `None` si la fenêtre n'existe plus ou si son processus est illisible.
#[must_use]
pub fn describe(handle: isize) -> Option<WindowInfo> {
    let process_id = window_process_id(handle)?;
    let path = process_image(process_id)?;
    Some(WindowInfo {
        handle,
        process_id,
        process: process_name_from_path(&path),
        title: window_title(handle),
    })
}

/// Lit la fenêtre actuellement au premier plan.
///
/// Retour : son identité.
///
/// # Errors
///
/// `NoForegroundWindow` s'il n'y a pas de fenêtre active, `UnreadableProcess` si son processus est illisible
/// (par exemple une fenêtre d'un processus plus privilégié).
pub fn foreground() -> Result<WindowInfo, Win32Error> {
    let handle = foreground_handle();
    if handle == 0 {
        return Err(Win32Error::NoForegroundWindow);
    }
    describe(handle).ok_or(Win32Error::UnreadableProcess)
}

/// Liste les fenêtres visibles de premier niveau dont l'identité est lisible.
///
/// Retour : leurs instantanés ; les fenêtres au processus illisible sont omises.
#[must_use]
pub fn visible_windows() -> Vec<WindowInfo> {
    visible_handles().into_iter().filter_map(describe).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_name_drops_folder_and_extension() {
        assert_eq!(
            process_name_from_path("C:\\Program Files\\Chrome\\chrome.exe"),
            "chrome"
        );
        assert_eq!(process_name_from_path("C:\\x\\NOTEPAD.EXE"), "NOTEPAD");
        assert_eq!(process_name_from_path("D:/tools/app.name.exe"), "app.name");
        assert_eq!(process_name_from_path("plain"), "plain");
        assert_eq!(process_name_from_path(".exe"), ".exe");
        assert_eq!(process_name_from_path(""), "");
    }

    #[test]
    fn foreground_reads_something_or_fails_cleanly() {
        match foreground() {
            Ok(window) => {
                assert_ne!(window.handle, 0);
                assert_ne!(window.process, "");
            }
            Err(error) => assert_ne!(error.to_string(), ""),
        }
    }

    #[test]
    fn visible_windows_have_readable_identities() {
        for window in visible_windows() {
            assert_ne!(window.process, "");
            assert_ne!(window.process_id, 0);
        }
    }

    #[test]
    fn unknown_window_is_not_described() {
        assert_eq!(describe(0), None);
    }
}
