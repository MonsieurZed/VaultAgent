//! Faux moteur scriptable pour les tests du serveur : réponses empilées, appels journalisés.
//!
//! Le journal contient les appels complets (mot de passe compris, dans un [`Secret`]) : à réserver aux tests,
//! avec des valeurs factices. Les réponses se servent dans l'ordre des appels, quelle que soit la méthode.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use vaultagent_core::{ErrorCode, Result};

use crate::engine::{Engine, EngineFactory, fail};
use crate::fake_call::Call;
use crate::fake_engine::FakeEngine;
use crate::fake_reply::Reply;

/// État partagé entre un faux moteur et sa poignée de test.
pub(crate) struct State {
    pub(crate) replies: VecDeque<Reply>,
    pub(crate) calls: Vec<Call>,
    pub(crate) alive: bool,
    pub(crate) kills: usize,
}

/// Verrouille un état partagé en ignorant l'empoisonnement.
///
/// Paramètres : le verrou.
/// Retour : la garde.
pub(crate) fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Poignée de test : script les réponses et lit ce que le moteur a reçu.
#[derive(Clone)]
pub struct FakeHandle(pub(crate) Arc<Mutex<State>>);

impl FakeHandle {
    /// Crée une poignée vivante, sans réponse en attente.
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(State {
            replies: VecDeque::new(),
            calls: Vec::new(),
            alive: true,
            kills: 0,
        })))
    }

    /// Empile la réponse du prochain appel.
    ///
    /// Paramètres : la réponse, dont la forme doit correspondre à la méthode appelée.
    /// Retour : la poignée, pour chaîner.
    #[allow(clippy::must_use_candidate)]
    pub fn push(&self, reply: Reply) -> &Self {
        locked(&self.0).replies.push_back(reply);
        self
    }

    /// Empile un succès sans valeur.
    ///
    /// Retour : la poignée, pour chaîner.
    #[allow(clippy::must_use_candidate)]
    pub fn push_ok(&self) -> &Self {
        self.push(Reply::Unit)
    }

    /// Empile un échec.
    ///
    /// Paramètres : le code d'erreur à rendre au prochain appel.
    /// Retour : la poignée, pour chaîner.
    #[allow(clippy::must_use_candidate)]
    pub fn push_error(&self, code: ErrorCode) -> &Self {
        self.push(Reply::Failure(code))
    }

    /// Appels reçus jusqu'ici, dans l'ordre.
    #[must_use]
    pub fn calls(&self) -> Vec<Call> {
        locked(&self.0).calls.clone()
    }

    /// Noms des méthodes appelées jusqu'ici, dans l'ordre.
    #[must_use]
    pub fn call_names(&self) -> Vec<&'static str> {
        locked(&self.0).calls.iter().map(Call::name).collect()
    }

    /// Nombre d'appels à `kill`.
    #[must_use]
    pub fn kills(&self) -> usize {
        locked(&self.0).kills
    }

    /// Simule la mort (ou la résurrection) du moteur.
    ///
    /// Paramètres : le nouvel état.
    pub fn set_alive(&self, alive: bool) {
        locked(&self.0).alive = alive;
    }

    /// Construit le moteur lié à cette poignée.
    #[must_use]
    pub fn engine(&self) -> FakeEngine {
        FakeEngine::new(self.clone())
    }
}

impl Default for FakeHandle {
    /// Équivaut à [`FakeHandle::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// État de la fabrique : démarrages à venir et passés.
struct FactoryState {
    queue: VecDeque<Result<FakeHandle>>,
    starts: Vec<(PathBuf, String)>,
}

/// Fabrique de faux moteurs, servis dans l'ordre où ils ont été préparés.
#[derive(Clone)]
pub struct FakeFactory(Arc<Mutex<FactoryState>>);

impl FakeFactory {
    /// Crée une fabrique sans moteur préparé.
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(FactoryState {
            queue: VecDeque::new(),
            starts: Vec::new(),
        })))
    }

    /// Prépare le prochain moteur à servir.
    ///
    /// Retour : sa poignée de test.
    #[must_use]
    pub fn engine(&self) -> FakeHandle {
        let handle = FakeHandle::new();
        locked(&self.0).queue.push_back(Ok(handle.clone()));
        handle
    }

    /// Prépare un échec de démarrage.
    ///
    /// Paramètres : le code d'erreur à rendre.
    pub fn push_failure(&self, code: &str) {
        locked(&self.0).queue.push_back(Err(fail(code)));
    }

    /// Démarrages demandés jusqu'ici : dossier et profil.
    #[must_use]
    pub fn starts(&self) -> Vec<(PathBuf, String)> {
        locked(&self.0).starts.clone()
    }
}

impl Default for FakeFactory {
    /// Équivaut à [`FakeFactory::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl EngineFactory for FakeFactory {
    /// Journalise le démarrage et sert le moteur suivant ; `engine-init` s'il n'en reste plus.
    fn start(&self, directory: &Path, profile: &str) -> Result<Box<dyn Engine>> {
        let mut state = locked(&self.0);
        state.starts.push((directory.to_path_buf(), profile.to_owned()));
        let handle = state.queue.pop_front().unwrap_or_else(|| Err(fail("engine-init")))?;
        Ok(Box::new(handle.engine()))
    }
}
