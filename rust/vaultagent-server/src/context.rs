//! Éléments partagés par toutes les routes : coffres, origine du service, proxy de confiance, horloge, aléa et attente.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use vaultagent_core::PairRequests;

use crate::clock::Clock;
use crate::pause::Pause;
use crate::shared_entropy::SharedEntropy;
use crate::vaults::Vaults;

/// Contexte d'une requête ; tout ce qui touche au temps ou à l'aléa y est injecté.
#[derive(Clone)]
pub struct Context {
    /// Coffres du serveur.
    pub vaults: Arc<Vaults>,
    /// Origine publique du service (`VAULTAGENT_ORIGIN`).
    pub origin: String,
    /// Adresse du proxy de confiance (`VAULTAGENT_TRUSTED_PROXY`).
    pub trusted_proxy: String,
    /// Propriétaire attendu dans `x-authentik-username`.
    pub owner: Option<String>,
    /// Horloge de la session.
    pub clock: Arc<dyn Clock>,
    /// Source d'aléa.
    pub entropy: SharedEntropy,
    /// Attente des relectures et du TOTP.
    pub pause: Arc<dyn Pause>,
    /// Demandes d'association en attente, communes à tous les coffres.
    pub pair_requests: Arc<Mutex<PairRequests>>,
    /// Dossier des archives de téléchargement, absent si la route est désactivée.
    pub downloads_directory: Option<PathBuf>,
}
