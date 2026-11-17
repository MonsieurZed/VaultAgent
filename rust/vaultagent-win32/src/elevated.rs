//! Lancement d'un programme avec élévation (`ShellExecuteExW`, verbe `runas`) : l'invite de contrôle de compte est la
//! preuve de présence de l'utilisateur. Attente bornée, fenêtre cachée.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, GetLastError, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, TerminateProcess, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

use crate::error::Win32Error;

/// Code de sortie donné à un processus élevé tué après le délai.
const KILLED_EXIT_CODE: u32 = 1;

/// Convertit un texte en UTF-16 terminé par un zéro.
fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

/// Demande l'exécution élevée et rend la poignée du processus créé.
///
/// Paramètres : le programme et sa ligne d'arguments, en UTF-16 terminés par zéro.
/// Retour : la poignée du processus (à fermer), `Cancelled` si l'invite est annulée, sinon l'erreur Windows.
///
/// Sûreté : la structure est initialisée à zéro puis remplie ; les trois chaînes restent vivantes pendant l'appel
/// et `SEE_MASK_NOCLOSEPROCESS` demande une poignée que l'appelant ferme.
fn start(exe: &[u16], args: &[u16]) -> Result<HANDLE, Win32Error> {
    let verb = wide(OsStr::new("runas"));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0);
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = exe.as_ptr();
    info.lpParameters = args.as_ptr();
    info.nShow = SW_HIDE;
    if unsafe { ShellExecuteExW(&raw mut info) } != 0 && !info.hProcess.is_null() {
        return Ok(info.hProcess);
    }
    match unsafe { GetLastError() } {
        ERROR_CANCELLED => Err(Win32Error::Cancelled),
        code => Err(Win32Error::Api {
            call: "ShellExecuteExW",
            code,
        }),
    }
}

/// Attend la fin du processus, le tue au-delà du délai, puis lit son code de sortie.
///
/// Paramètres : la poignée du processus et le délai maximal.
/// Retour : le code de sortie, ou `TimedOut` si le processus a dû être tué.
///
/// Sûreté : la poignée vient de `start` et reste ouverte jusqu'à l'appel de `CloseHandle` par l'appelant ; le code
/// de sortie est écrit dans une variable locale.
fn wait(process: HANDLE, timeout: Duration) -> Result<u32, Win32Error> {
    let milliseconds = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
    if unsafe { WaitForSingleObject(process, milliseconds) } != WAIT_OBJECT_0 {
        unsafe { TerminateProcess(process, KILLED_EXIT_CODE) };
        return Err(Win32Error::TimedOut);
    }
    let mut code = 0u32;
    if unsafe { GetExitCodeProcess(process, &raw mut code) } == 0 {
        return Err(Win32Error::Api {
            call: "GetExitCodeProcess",
            code: unsafe { GetLastError() },
        });
    }
    Ok(code)
}

/// Lance un programme avec élévation (invite UAC) et attend sa fin.
///
/// Paramètres : le programme, sa ligne d'arguments et le délai maximal (le processus est tué au-delà).
/// Retour : le code de sortie du processus élevé.
///
/// Sûreté : la poignée rendue par `start` est fermée une seule fois ici, quel que soit le résultat de l'attente.
///
/// # Errors
///
/// `Win32Error::Cancelled` si l'utilisateur annule l'invite, `Win32Error::TimedOut` après le délai,
/// `Win32Error::Api` pour tout autre refus.
pub fn run_elevated(exe: &Path, args: &str, timeout: Duration) -> Result<u32, Win32Error> {
    let process = start(&wide(exe.as_os_str()), &wide(OsStr::new(args)))?;
    let result = wait(process, timeout);
    unsafe { CloseHandle(process) };
    result
}
