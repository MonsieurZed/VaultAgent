//! Client `VaultAgent` (bibliothèque et binaire `vaultagent`) : association, lecture, écriture, déverrouillage
//! et remplissage. Sorties JSON versionnées et codes de sortie stables.
//!
//! Tâches 3.1 à 3.6 : configuration privée, client HTTPS, association par approbation web, lectures, écritures,
//! déverrouillage, verrouillage, `seal`, remplissage par enveloppe, aide et codes de sortie.

#![forbid(unsafe_code)]

pub mod api;
pub mod cli;
pub mod config;
pub mod config_path;
pub mod error;
pub mod exit_code;
pub mod fake_transport;
pub mod fill;
pub mod help;
pub mod https_transport;
pub mod machine_name;
pub mod pairing;
pub mod pairing_wire;
pub mod pause;
pub mod private_file;
pub mod reads;
pub mod run;
pub mod seal;
pub mod server_address;
#[cfg(test)]
mod test_dir;
pub mod token;
pub mod transport;
pub mod unlock;
pub mod write;
pub mod write_args;
pub(crate) mod write_body;
pub mod write_metadata;
pub mod write_values;
