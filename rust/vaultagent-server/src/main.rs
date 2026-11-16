//! Binaire `vaultagent-server` : lit l'environnement, assemble le service avec le moteur natif et écoute.

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::sync::Arc;

use vaultagent_engine::SdkEngineFactory;
use vaultagent_server::bootstrap::build_context;
use vaultagent_server::error::Result;
use vaultagent_server::settings::Settings;
use vaultagent_server::transport::serve;

/// Démarre le service et attend son arrêt.
///
/// Paramètres : aucun.
/// Retour : une erreur de démarrage.
fn run() -> Result<()> {
    let settings = Settings::from_lookup(|name| std::env::var(name).ok())?;
    let context = build_context(&settings, Arc::new(SdkEngineFactory))?;
    let running = serve(
        &format!("{}:{}", settings.bind, settings.port),
        settings.workers,
        &context,
    )?;
    println!("VaultAgent Linux ready; vault locked");
    running.wait();
    Ok(())
}

/// Point d'entrée : code de sortie 1 et message sur l'erreur standard si le démarrage échoue.
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vaultagent-server : {error}");
            ExitCode::FAILURE
        }
    }
}
