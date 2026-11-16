//! Source d'aléa partageable entre sessions et coffres : un `Arc<Mutex<…>>` qui implémente [`Entropy`].

use std::sync::{Arc, Mutex};

use vaultagent_core::{Entropy, Result, SystemEntropy};

use crate::sync::locked;

/// Aléa partagé ; les clones tirent dans la même source.
#[derive(Clone)]
pub struct SharedEntropy(Arc<Mutex<Box<dyn Entropy + Send>>>);

impl SharedEntropy {
    /// Enveloppe une source.
    ///
    /// Paramètres : la source (déterministe dans les tests).
    /// Retour : la source partageable.
    #[must_use]
    pub fn new(source: Box<dyn Entropy + Send>) -> Self {
        Self(Arc::new(Mutex::new(source)))
    }

    /// Source du système d'exploitation.
    ///
    /// Paramètres : aucun.
    /// Retour : la source partageable.
    #[must_use]
    pub fn system() -> Self {
        Self::new(Box::new(SystemEntropy))
    }
}

impl Entropy for SharedEntropy {
    /// Remplit le tampon depuis la source partagée.
    fn fill(&mut self, buffer: &mut [u8]) -> Result<()> {
        locked(&self.0).fill(buffer)
    }
}
