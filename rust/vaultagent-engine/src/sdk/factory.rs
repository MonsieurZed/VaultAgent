//! Fabrique des moteurs natifs : un moteur par profil, état du SDK en mémoire.

use std::path::Path;

use vaultagent_core::Result;

use crate::engine::{Engine, EngineFactory};
use crate::sdk::engine::SdkEngine;

/// Fabrique de moteurs sur le SDK Bitwarden.
///
/// Le SDK garde ses dépôts et ses clés en mémoire : le dossier de données reçu n'est ni lu ni écrit.
#[derive(Debug, Clone, Copy, Default)]
pub struct SdkEngineFactory;

impl EngineFactory for SdkEngineFactory {
    /// Démarre un moteur non connecté.
    ///
    /// Paramètres : le dossier de données et le profil, tous deux ignorés (état en mémoire).
    /// Retour : le moteur.
    fn start(&self, _directory: &Path, _profile: &str) -> Result<Box<dyn Engine>> {
        SdkEngine::new().map(|engine| Box::new(engine) as Box<dyn Engine>)
    }
}
