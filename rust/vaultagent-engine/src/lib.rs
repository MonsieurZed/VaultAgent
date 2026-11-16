//! Moteur Bitwarden de `VaultAgent` : trait [`Engine`] typé (une méthode par opération), types de domaine
//! (`Item`, `Folder`, `Organization`, `Passkey`, …) et faux moteur scriptable des tests du serveur.
//!
//! L'interface est bloquante. Le moteur natif sur le SDK Bitwarden (tâches 2.1c et 2.1d) la réalise derrière une
//! façade `async` confinée à cette crate ; le pilote du processus Node de `fork/agent.ts` n'existe plus. Le module
//! `fake` est toujours compilé : les tests des autres crates ne peuvent pas activer un `cfg(test)` de celle-ci, et
//! il ne dépend de rien d'autre que la crate.

#![forbid(unsafe_code)]

mod engine;
pub mod fake;
mod fake_call;
mod fake_engine;
mod fake_reply;
mod item;
mod passkey;
mod sdk;
mod secret;
mod structure;

pub use engine::{Account, Engine, EngineFactory, fail, stage_error};
pub use fake::{FakeFactory, FakeHandle};
pub use fake_call::Call;
pub use fake_engine::FakeEngine;
pub use fake_reply::Reply;
pub use item::{Item, Login, LoginUri, PasswordHistoryEntry};
pub use passkey::{Passkey, PasskeyEntry};
pub use sdk::{SdkEngine, SdkEngineFactory};
pub use secret::Secret;
pub use structure::{Collection, Folder, Organization};
