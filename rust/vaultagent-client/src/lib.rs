//! Client `VaultAgent` (bibliothèque et binaire `vaultagent`) : association, lecture, écriture, déverrouillage
//! et remplissage. Sorties JSON versionnées et codes de sortie stables.
//!
//! Tâche 4.5b : application de barre `tray` : logique pure (`tray_select`, `tray_menu`, `tray_status`,
//! `tray_actions`, `elevation_disable`, `translate`, `iso_time`) et interface Win32 (Windows seulement).
//!
//! Tâche 4.4 : Windows Hello (`hello_system`, `hello_cose`, `hello_proof`), `hello-status` et `hello-enroll`, preuve
//! Hello des décisions.
//!
//! Tâche 4.3 : clé d'élévation (DPAPI machine), signature par processus élevé, `decide`, `approvals`,
//! `elevation-enroll` et commande cachée `elevation-sign`.
//!
//! Tâche 4.2 : configuration protégée par DPAPI (`remote-client.json`) et clé d'approbateur (`approval-key.dpapi`).
//!
//! Tâche 4.1 : commandes de fenêtre `foreground`, `focus` et `type` (Windows), règles pures de la cible.
//!
//! Tâches 3.1 à 3.6 : configuration privée, client HTTPS, association par approbation web, lectures, écritures,
//! déverrouillage, verrouillage, `seal`, remplissage par enveloppe, aide et codes de sortie.

#![forbid(unsafe_code)]

pub mod api;
#[cfg(windows)]
pub mod approval_card;
pub mod approval_commands;
pub mod approval_request;
pub mod approver_key;
pub mod cli;
pub mod config;
pub mod config_path;
pub mod config_store;
pub mod decide;
pub mod elevated_sign;
pub mod elevation_disable;
pub mod elevation_enroll;
pub mod elevation_key;
pub mod elevation_sign;
pub mod elevation_system;
pub mod error;
pub mod exit_code;
pub mod fake_elevation;
pub mod fake_hello;
pub mod fake_transport;
pub mod fill;
pub mod hello_cose;
mod hello_decide_tests;
pub mod hello_enroll;
mod hello_enroll_tests;
mod hello_fixture;
pub mod hello_proof;
pub mod hello_system;
pub mod help;
pub mod https_transport;
pub mod iso_time;
pub mod machine_name;
#[cfg(windows)]
pub mod pair_window;
pub mod pairing;
pub mod pairing_flow;
pub mod pairing_wire;
pub mod pause;
pub mod private_file;
pub mod proofs;
pub mod protected_config;
pub mod protector;
pub mod reads;
pub mod run;
pub mod seal;
pub mod server_address;
pub mod target_check;
pub mod target_report;
pub mod target_requirement;
#[cfg(test)]
mod test_dir;
pub mod text_fold;
pub mod token;
pub mod translate;
pub mod transport;
pub mod tray_actions;
#[cfg(windows)]
pub mod tray_app;
#[cfg(windows)]
pub mod tray_clock;
#[cfg(windows)]
pub mod tray_jobs;
pub mod tray_menu;
#[cfg(windows)]
pub mod tray_run;
pub mod tray_select;
pub mod tray_status;
#[cfg(windows)]
pub mod tray_worker;
pub mod unlock;
pub mod window_args;
pub mod window_commands;
pub mod window_system;
#[cfg(windows)]
pub mod windows_elevation;
#[cfg(windows)]
pub mod windows_hello;
#[cfg(windows)]
pub mod windows_system;
pub mod write;
pub mod write_args;
pub(crate) mod write_body;
pub mod write_metadata;
pub mod write_values;
