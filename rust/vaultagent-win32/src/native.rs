//! Appels Win32 bruts de lecture : fenêtre au premier plan, identité et énumération. Seul endroit de lecture `unsafe`.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

/// Taille du tampon du chemin d'image, en unités UTF-16 (limite étendue de Windows).
const PATH_CAPACITY: usize = 32_768;

/// Convertit un identifiant de fenêtre entier en `HWND`.
///
/// Paramètres : l'identifiant.
/// Retour : le `HWND` correspondant (non déréférencé).
pub(crate) fn hwnd(handle: isize) -> HWND {
    handle as HWND
}

/// Lit la fenêtre au premier plan.
///
/// Retour : son identifiant, ou 0 s'il n'y en a pas.
///
/// Sûreté : `GetForegroundWindow` n'a aucun paramètre ni précondition.
pub(crate) fn foreground_handle() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

/// Lit l'identifiant du processus propriétaire d'une fenêtre.
///
/// Paramètres : l'identifiant de fenêtre.
/// Retour : le pid, ou `None` si la fenêtre n'existe plus.
///
/// Sûreté : le pointeur de sortie vise une variable locale valide pour toute la durée de l'appel.
pub(crate) fn window_process_id(handle: isize) -> Option<u32> {
    let mut pid: u32 = 0;
    let thread = unsafe { GetWindowThreadProcessId(hwnd(handle), &raw mut pid) };
    (thread != 0 && pid != 0).then_some(pid)
}

/// Lit la longueur du titre d'une fenêtre.
///
/// Paramètres : l'identifiant de fenêtre.
/// Retour : le nombre d'unités UTF-16 du titre, sans le zéro final.
///
/// Sûreté : `GetWindowTextLengthW` accepte tout `HWND` et rend 0 s'il est invalide.
fn title_length(handle: isize) -> usize {
    usize::try_from(unsafe { GetWindowTextLengthW(hwnd(handle)) }).unwrap_or(0)
}

/// Copie le titre d'une fenêtre dans un tampon.
///
/// Paramètres : l'identifiant de fenêtre, le tampon qui reçoit le titre et un zéro final.
/// Retour : le nombre d'unités UTF-16 copiées.
///
/// Sûreté : le pointeur et la capacité viennent du même tampon `&mut [u16]`, valide pendant l'appel.
fn copy_title(handle: isize, buffer: &mut [u16]) -> usize {
    let capacity = i32::try_from(buffer.len()).unwrap_or(i32::MAX);
    let copied = unsafe { GetWindowTextW(hwnd(handle), buffer.as_mut_ptr(), capacity) };
    usize::try_from(copied).unwrap_or(0)
}

/// Lit le titre d'une fenêtre.
///
/// Paramètres : l'identifiant de fenêtre.
/// Retour : le titre, vide si la fenêtre n'en a pas ou n'existe plus.
pub(crate) fn window_title(handle: isize) -> String {
    let mut buffer = vec![0u16; title_length(handle) + 1];
    let copied = copy_title(handle, &mut buffer);
    String::from_utf16_lossy(&buffer[..copied.min(buffer.len())])
}

/// Ouvre un processus en lecture limitée.
///
/// Paramètres : le pid.
/// Retour : le handle, ou 0 si l'accès est refusé.
///
/// Sûreté : `OpenProcess` n'a pas de pointeur ; le handle rendu est fermé par `close_handle`.
fn open_process(pid: u32) -> isize {
    unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) as isize }
}

/// Ferme un handle de processus.
///
/// Paramètres : le handle rendu par `open_process`.
///
/// Sûreté : le handle est valide et n'est plus utilisé après cet appel.
fn close_handle(handle: isize) {
    unsafe {
        CloseHandle(handle as *mut c_void);
    }
}

/// Lit le chemin complet de l'image d'un processus ouvert.
///
/// Paramètres : le handle du processus.
/// Retour : le chemin, ou `None` si la lecture échoue.
///
/// Sûreté : le tampon et la taille en entrée désignent le même vecteur ; la taille est mise à jour par l'appel.
fn image_path(process: isize) -> Option<String> {
    let mut buffer = vec![0u16; PATH_CAPACITY];
    let mut size = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
    let ok = unsafe { QueryFullProcessImageNameW(process as *mut c_void, 0, buffer.as_mut_ptr(), &raw mut size) };
    (ok != 0).then(|| String::from_utf16_lossy(&buffer[..usize::try_from(size).unwrap_or(0).min(buffer.len())]))
}

/// Lit le chemin de l'image du processus d'un pid.
///
/// Paramètres : le pid.
/// Retour : le chemin complet, ou `None` si le processus n'est pas lisible.
pub(crate) fn process_image(pid: u32) -> Option<String> {
    let process = open_process(pid);
    if process == 0 {
        return None;
    }
    let path = image_path(process);
    close_handle(process);
    path
}

/// Rappel d'`EnumWindows` : ajoute à la liste les fenêtres visibles.
///
/// Sûreté : `parameter` est le pointeur vers le `Vec<isize>` passé par `visible_handles`, vivant et unique
/// pendant toute l'énumération ; Windows appelle ce rappel sur le même fil, avant le retour d'`EnumWindows`.
unsafe extern "system" fn collect_visible(window: HWND, parameter: LPARAM) -> i32 {
    let list = unsafe { &mut *(parameter as *mut Vec<isize>) };
    if unsafe { IsWindowVisible(window) } != 0 {
        list.push(window as isize);
    }
    1
}

/// Énumère les fenêtres visibles de premier niveau.
///
/// Retour : leurs identifiants, dans l'ordre de l'ordre Z de Windows.
///
/// Sûreté : le pointeur vers le vecteur local reste valide pendant l'appel synchrone à `EnumWindows`.
pub(crate) fn visible_handles() -> Vec<isize> {
    let mut list: Vec<isize> = Vec::new();
    unsafe {
        EnumWindows(Some(collect_visible), &raw mut list as LPARAM);
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_window_has_no_process_and_no_title() {
        assert_eq!(window_process_id(0), None);
        assert_eq!(window_title(0), "");
    }

    #[test]
    fn own_process_image_is_readable() {
        let path = process_image(std::process::id());
        assert!(path.is_some_and(|text| !text.is_empty()));
    }
}
