//! Outil de parité : rejoue le même scénario sur le serveur Node et le serveur Rust (tunnels ssh vers les ports de
//! test) et compare statuts, en-têtes utiles et corps normalisés. Code de sortie 0 si tout concorde, 1 sinon, 2 si
//! l'usage ou le réseau échoue.

#![forbid(unsafe_code)]

mod accepted;
mod args;
mod diff;
mod error;
mod exchange;
mod http;
mod normalize;
mod report;
mod run;
mod scenario;
mod scenario_admin;
mod scenario_agent;
mod scenario_errors;
mod scenario_lock;
mod scenario_pair;

use error::Result;
use run::{Run, Settings};
use std::process::ExitCode;
use std::time::Duration;
use vaultagent_core::encoding::hex_encode;
use vaultagent_core::{Entropy, SystemEntropy, generate_rsa, public_key_spki};

const KEY_BITS: usize = 2048;

/// Prépare les valeurs communes, joue le scénario sur les deux serveurs et compare.
///
/// Paramètres : les arguments sans le nom du programme.
/// Retour : le code de sortie.
/// # Errors
///
/// Une erreur d'usage ou de cryptographie du cœur.
fn execute(arguments: &[String]) -> Result<ExitCode> {
    let config = args::parse(arguments)?;
    let key = generate_rsa(KEY_BITS, &mut SystemEntropy)?;
    let public_key = public_key_spki(&key)?;
    let mut tag_bytes = [0u8; 4];
    SystemEntropy.fill(&mut tag_bytes)?;
    let settings = Settings {
        tag: hex_encode(&tag_bytes),
        owner: config.owner,
        server: config.server,
        email: config.email,
        password: config.password,
        key,
        public_key,
    };
    let mut node = Run::new(&config.node.address, &config.node.origin, &settings);
    scenario::play(&mut node);
    std::thread::sleep(Duration::from_secs(config.pause));
    let mut rust = Run::new(&config.rust.address, &config.rust.origin, &settings);
    scenario::play(&mut rust);
    if config.trace {
        print!("{}", report::trace(&node.exchanges, &rust.exchanges));
    }
    let found = diff::compare(&node.exchanges, &rust.exchanges);
    let (refused, accepted) = accepted::split(accepted::ACCEPTED, found);
    let (text, summary) = report::render(&refused, &accepted, node.exchanges.len().max(rust.exchanges.len()));
    print!("{text}");
    Ok(if summary.refused == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// Point d'entrée.
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    execute(&arguments).unwrap_or_else(|error| {
        eprintln!("{error}");
        ExitCode::from(2)
    })
}
