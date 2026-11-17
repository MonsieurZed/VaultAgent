//! Seule crate de `VaultAgent` où `unsafe` est autorisé (déclaré dans son `Cargo.toml`) : identité et premier plan
//! des fenêtres, `SendInput`, DPAPI, clés `NCrypt`, WebAuthn Hello, icône de barre et carte d'approbation.
//! Elle n'expose que des fonctions sûres ; chaque bloc `unsafe` porte un docblock `Sûreté :`.
//!
//! Tâche 4.1 : fenêtre au premier plan et son identité (`window`), mise au premier plan (`raise`), frappe `SendInput`
//! contrôlée caractère par caractère (`input`).
//!
//! Tâche 4.2 : protection DPAPI des secrets (`dpapi`), portées utilisateur courant et machine.
//!
//! Tâche 4.3 : identité du processus (`identity`), dossier protégé (`private_dir`), lancement élevé (`elevated`).
//!
//! Tâche 4.5b : interface native (`ui_window`, `ui_controls`, `ui_tray`, `ui_menu`, `ui_icon`, `ui_system`) : fenêtres,
//! contrôles, icône de barre, menu, bouclier en mémoire, instance unique, registre.
//!
//! Tâche 4.4 : Windows Hello WebAuthn (`hello`) : disponibilité, enregistrement, signature d'un défi.

#![cfg(windows)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod dpapi;
pub mod elevated;
pub mod error;
pub mod hello;
mod hello_native;
pub mod identity;
pub mod input;
mod native;
pub mod private_dir;
pub mod raise;
pub mod ui_controls;
pub mod ui_icon;
pub mod ui_menu;
mod ui_ref;
mod ui_system;
mod ui_text;
pub mod ui_tray;
pub mod ui_window;
pub mod window;

#[cfg(test)]
mod live_tests;
#[cfg(test)]
mod test_window;
#[cfg(test)]
mod ui_live_tests;

pub use dpapi::{Scope, protect, unprotect};
pub use elevated::run_elevated;
pub use error::Win32Error;
pub use hello::{
    Assertion, HelloAvailability, Registration, hello_available, hello_register, hello_sign, hello_window,
};
pub use identity::{current_user_sid, is_elevated};
pub use input::{TypeError, type_text};
pub use private_dir::create_private_directory;
pub use raise::bring_to_front;
pub use ui_controls::Control;
pub use ui_icon::{ICON_SIZE, Icon, shield_pixels};
pub use ui_menu::{MenuEntry, show_menu};
pub use ui_ref::{quit, run_message_loop};
pub use ui_system::{SingleInstance, open_url, registry_text, user_locale_name, work_area};
pub use ui_tray::{BalloonKind, TrayIcon};
pub use ui_window::{Poster, TrayAction, UiEvent, UiHandler, WindowKind, WindowRef, WindowSpec, create_window};
pub use window::{WindowInfo, describe, foreground, process_name_from_path, visible_windows};
