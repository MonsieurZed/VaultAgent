//! Point d'entrée de la commande `tray` : instance unique, langue, fil réseau, fenêtre cachée et boucle de messages.

use std::collections::VecDeque;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use vaultagent_win32::{
    Icon, SingleInstance, WindowKind, WindowSpec, create_window, registry_text, run_message_loop, user_locale_name,
};

use crate::error::ClientError;
use crate::exit_code::ExitCode;
use crate::translate::{Language, Translator};
use crate::tray_app::TrayApp;
use crate::tray_jobs::Job;
use crate::tray_worker::{WorkerSetup, spawn};

const INSTANCE_NAME: &str = "Local\\VaultAgentTrayClient";
const PREFERENCES_KEY: &str = "Software\\MonsieurZed\\VaultAgent\\Preferences";

/// Choisit la langue : préférence du registre, sinon langue de Windows.
///
/// Retour : la langue de l'interface.
#[must_use]
pub fn interface_language() -> Language {
    let preference = registry_text(PREFERENCES_KEY, "Language");
    Language::choose(preference.as_deref(), &user_locale_name())
}

/// Lance l'application de barre et attend sa fermeture.
///
/// Paramètres : le poste (stockage, dossiers, nom de machine) ; la langue du poste est ajoutée ici.
/// Retour : `Success` à la fermeture ou si une autre instance tourne déjà ; une erreur système est rendue en
/// `ClientError::Window`.
///
/// # Errors
///
/// `Window` si l'icône, la fenêtre ou l'instance unique échouent.
pub fn run_tray(mut setup: WorkerSetup) -> Result<ExitCode, ClientError> {
    let window_error = |error: vaultagent_win32::Win32Error| ClientError::Window(error.to_string());
    let Some(_instance) = SingleInstance::acquire(INSTANCE_NAME).map_err(window_error)? else {
        return Ok(ExitCode::Success);
    };
    setup.language = interface_language();
    let icon = Icon::shield().map_err(window_error)?;
    let (sender, receiver) = channel::<Job>();
    let mailbox = Arc::new(Mutex::new(VecDeque::new()));
    let app = TrayApp::new(Translator::new(setup.language), icon, sender.clone(), mailbox.clone());
    let spec = WindowSpec {
        class: "VaultAgentTrayWindow",
        title: "VaultAgent",
        kind: WindowKind::Hidden,
        rect: (0, 0, 10, 10),
    };
    let window = create_window(&spec, Box::new(app)).map_err(window_error)?;
    let worker = spawn(setup, receiver, mailbox, window.poster());
    run_message_loop();
    sender.send(Job::Stop).ok();
    drop(worker);
    Ok(ExitCode::Success)
}
