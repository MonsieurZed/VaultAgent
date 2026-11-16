//! Moteur natif sur le SDK Bitwarden : `SdkEngine` (trait `Engine`) et `SdkEngineFactory`.
//!
//! Connexion avec second facteur, verrouillage par destruction du client, synchronisation, lectures, TOTP,
//! dossiers, organisations et contrôle du partage (tâche 2.1c) ; création, mise à jour avec contrôle de révision,
//! corbeille, restauration, collections, partage et passkeys (tâche 2.1d). L'état du SDK reste en mémoire.

mod clock;
mod convert;
mod engine;
#[cfg(test)]
mod engine_tests;
mod errors;
mod factory;
mod filter;
mod folders;
mod login;
mod org;
mod passkeys;
mod patch;
mod read;
mod session;
mod share;
mod sync_handler;
mod write;
mod write_errors;

pub use engine::SdkEngine;
pub use factory::SdkEngineFactory;
