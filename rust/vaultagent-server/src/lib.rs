//! Serveur `VaultAgent` (bibliothèque et binaire) : magasin chiffré, session, routes `/api/agent/*`,
//! `/api/admin/*`, `/api/pair`, front HTML sans JavaScript et téléchargements. Cible Linux.
//!
//! Tâche 2.2 : magasin fichier ([`Store`]), session à horloge et moteur injectés ([`Session`]) et coffres
//! ([`Vaults`]). Tâche 2.3 : cœur HTTP pur [`route`] et routes agent. Tâche 2.4a : routes d'administration, association,
//! téléchargements, réglages ([`settings`]) et transport `tiny_http` ([`transport`]) du binaire. Tâche 2.4b : front rendu
//! par Rust (`front_*`, `i18n*`), formulaires `POST /admin/<action>`, sans aucun script.

#![forbid(unsafe_code)]

pub mod account;
mod admin;
mod admin_devices;
mod admin_state;
mod agent;
mod agent_auth;
mod agent_lifecycle;
mod agent_read;
mod agent_seal;
mod agent_share;
mod agent_status;
mod agent_unlock;
mod agent_write;
pub mod bootstrap;
pub mod clock;
pub mod configure_input;
pub mod context;
mod engine_slot;
pub mod error;
mod files;
mod form;
mod front_about;
mod front_account;
mod front_action;
mod front_body;
mod front_clients;
mod front_dates;
mod front_devices;
mod front_escape;
mod front_html;
mod front_lang;
mod front_notice;
mod front_page;
mod front_pairing;
mod front_response;
mod front_status;
mod front_tab;
mod front_view;
mod guard;
pub mod healthcheck;
pub mod http;
mod i18n;
mod i18n_account;
mod i18n_clients;
mod i18n_layout;
pub mod item_value;
mod owner;
mod pair_route;
pub mod pause;
pub mod private_fs;
pub mod query;
pub mod route;
pub mod session;
mod session_configure;
pub mod settings;
pub mod shared_entropy;
pub mod store;
mod store_files;
mod sync;
pub mod transport;
mod transport_request;
mod transport_response;
pub mod vaults;
mod verify_loop;
mod write_ledger;

pub use account::Account;
pub use clock::{Clock, ManualClock, SystemClock};
pub use configure_input::ConfigureInput;
pub use context::Context;
pub use engine_slot::EngineSlot;
pub use error::{Result, ServerError};
pub use http::{Request, Response};
pub use pause::{Pause, RecordedPause, ThreadPause};
pub use route::route;
pub use session::{Session, SessionStatus, VERSION};
pub use shared_entropy::SharedEntropy;
pub use store::Store;
pub use vaults::{VaultContext, Vaults};
