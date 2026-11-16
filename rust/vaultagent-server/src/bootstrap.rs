//! Assemblage du service à partir des réglages : magasin, coffres, horloge, aléa et contexte des routes.

use std::sync::{Arc, Mutex};

use vaultagent_core::PairRequests;
use vaultagent_engine::EngineFactory;

use crate::clock::SystemClock;
use crate::context::Context;
use crate::error::Result;
use crate::pause::ThreadPause;
use crate::settings::Settings;
use crate::shared_entropy::SharedEntropy;
use crate::store::Store;
use crate::vaults::Vaults;

/// Construit le contexte des routes ; tous les coffres démarrent verrouillés.
///
/// Paramètres : les réglages, la fabrique de moteurs (le moteur natif en production, le faux moteur aux essais).
/// Retour : le contexte.
///
/// # Errors
///
/// Erreur d'ouverture du magasin, `invalid-vault-registry` ou `invalid-vault`.
pub fn build_context(settings: &Settings, factory: Arc<dyn EngineFactory>) -> Result<Context> {
    let clock = Arc::new(SystemClock::new());
    let entropy = SharedEntropy::system();
    let root = Arc::new(Store::open(&settings.data)?);
    let vaults = Arc::new(Vaults::open(root, factory, clock.clone(), entropy.clone())?);
    Ok(Context {
        vaults,
        origin: settings.origin.clone(),
        trusted_proxy: settings.trusted_proxy.clone(),
        owner: Some(settings.owner.clone()),
        clock,
        entropy,
        pause: Arc::new(ThreadPause),
        pair_requests: Arc::new(Mutex::new(PairRequests::new())),
        downloads_directory: settings.downloads.clone(),
    })
}
