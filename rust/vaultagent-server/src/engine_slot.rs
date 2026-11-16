//! Moteur partagé d'une session : requêtes sérialisées, arrêt possible pendant une requête en cours.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use vaultagent_core::ErrorCode;
use vaultagent_engine::Engine;

use crate::error::Result;
use crate::sync::locked;

/// Moteur connecté. Un verrouillage de la session le marque condamné : si une requête le tient, elle l'arrête
/// à sa fin, et aucune requête ne part plus.
pub struct EngineSlot {
    engine: Mutex<Box<dyn Engine>>,
    doomed: AtomicBool,
}

impl EngineSlot {
    /// Enveloppe un moteur connecté.
    ///
    /// Paramètres : le moteur.
    /// Retour : l'emplacement.
    #[must_use]
    pub fn new(engine: Box<dyn Engine>) -> Self {
        Self {
            engine: Mutex::new(engine),
            doomed: AtomicBool::new(false),
        }
    }

    /// Exécute une opération sur le moteur, seule : les opérations d'un moteur se sérialisent.
    ///
    /// Paramètres : la fonction qui reçoit le moteur et appelle sa méthode (`engine.sync()`, …).
    /// Retour : le résultat de la fonction.
    ///
    /// # Errors
    ///
    /// `locked` si le moteur est condamné ; sinon l'erreur du moteur (`engine-*`, `<étape>:<catégorie>`).
    pub fn run<T>(&self, operation: impl FnOnce(&mut dyn Engine) -> vaultagent_core::Result<T>) -> Result<T> {
        if self.doomed.load(Ordering::SeqCst) {
            return Err(ErrorCode::Locked.into());
        }
        let result = operation(&mut **locked(&self.engine));
        self.reap();
        Ok(result?)
    }

    /// Indique si le moteur tourne encore ; un moteur occupé par une requête est tenu pour vivant.
    ///
    /// Paramètres : aucun.
    /// Retour : `true` si le moteur est utilisable.
    pub(crate) fn alive(&self) -> bool {
        if self.doomed.load(Ordering::SeqCst) {
            return false;
        }
        match self.engine.try_lock() {
            Ok(mut engine) => engine.alive(),
            Err(std::sync::TryLockError::WouldBlock) => true,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner().alive(),
        }
    }

    /// Condamne le moteur et l'arrête tout de suite s'il est libre ; sinon le fil qui le tient l'arrête à la fin de
    /// sa requête.
    ///
    /// Paramètres : aucun.
    pub(crate) fn kill(&self) {
        self.doomed.store(true, Ordering::SeqCst);
        match self.engine.try_lock() {
            Ok(mut engine) => engine.kill(),
            Err(std::sync::TryLockError::WouldBlock) => {}
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner().kill(),
        }
    }

    /// Arrête le moteur s'il a été condamné pendant une requête.
    ///
    /// Paramètres : aucun.
    fn reap(&self) {
        if self.doomed.load(Ordering::SeqCst) {
            locked(&self.engine).kill();
        }
    }
}
