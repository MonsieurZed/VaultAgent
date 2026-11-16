//! Session d'un coffre : moteur connecté, délai d'inactivité glissant, génération qui invalide les opérations en
//! cours. `configure` et `unlock` sont dans `session_configure`.

use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use vaultagent_core::SessionControl;
use vaultagent_core::device_record::iso_from_millis;
use vaultagent_engine::EngineFactory;
use zeroize::Zeroizing;

use crate::clock::Clock;
use crate::engine_slot::EngineSlot;
use crate::error::{Result, ServerError};
use crate::shared_entropy::SharedEntropy;
use crate::store::Store;
use crate::sync::locked;

/// Version annoncée par `status`.
pub const VERSION: &str = "3.0.0-preview.7";

/// État rendu par `status`, sans aucun secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    /// Version du service.
    pub version: &'static str,
    /// Un compte est enregistré.
    pub configured: bool,
    /// Le moteur est connecté.
    pub unlocked: bool,
    /// Délai d'inactivité en minutes, `null` s'il est désactivé ou si rien n'est configuré.
    pub timeout_minutes: Option<u32>,
    /// Échéance ISO du délai, `null` si verrouillé ou sans délai.
    pub expires_utc: Option<String>,
}

pub(crate) struct State {
    pub(crate) generation: u64,
    pub(crate) engine: Option<Arc<EngineSlot>>,
    pub(crate) deadline: Option<u64>,
}

/// Session d'un coffre ; partageable entre fils, les opérations longues se sérialisent par [`Session::serial`].
pub struct Session {
    pub(crate) store: Arc<Store>,
    pub(crate) factory: Arc<dyn EngineFactory>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) entropy: SharedEntropy,
    pub(crate) state: Mutex<State>,
    serial: Mutex<()>,
}

impl Session {
    /// Crée une session verrouillée.
    ///
    /// Paramètres : le magasin, la fabrique de moteurs, l'horloge, la source d'aléa.
    /// Retour : la session.
    #[must_use]
    pub fn new(
        store: Arc<Store>,
        factory: Arc<dyn EngineFactory>,
        clock: Arc<dyn Clock>,
        entropy: SharedEntropy,
    ) -> Self {
        Self {
            store,
            factory,
            clock,
            entropy,
            state: Mutex::new(State {
                generation: 0,
                engine: None,
                deadline: None,
            }),
            serial: Mutex::new(()),
        }
    }

    /// Magasin de la session.
    ///
    /// Paramètres : aucun.
    /// Retour : le magasin.
    #[must_use]
    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    /// Verrou qui sérialise les opérations d'une session (`configure`, `unlock`, lectures, écritures). Le
    /// verrouillage de la session ne le prend pas : il peut donc invalider une opération en cours.
    ///
    /// Paramètres : aucun.
    /// Retour : la garde, à garder pendant toute l'opération.
    pub fn serial(&self) -> MutexGuard<'_, ()> {
        locked(&self.serial)
    }

    /// Génération courante, incrémentée à chaque verrouillage.
    ///
    /// Paramètres : aucun.
    /// Retour : le compteur.
    pub fn generation(&self) -> u64 {
        locked(&self.state).generation
    }

    /// État de la session ; verrouille d'abord si le délai est échu, et ne prolonge jamais le délai.
    ///
    /// Paramètres : aucun.
    /// Retour : le statut.
    pub fn status(&self) -> SessionStatus {
        let mut state = locked(&self.state);
        self.expire_in(&mut state);
        let account = self.store.account();
        let now = self.clock.monotonic_ms();
        let expires = match (&state.engine, state.deadline) {
            (Some(_), Some(deadline)) => Some(iso_from_millis(self.clock.wall_ms() + deadline.saturating_sub(now))),
            _ => None,
        };
        SessionStatus {
            version: VERSION,
            configured: account.is_some(),
            unlocked: state.engine.is_some(),
            timeout_minutes: account.and_then(|value| value.timeout_minutes),
            expires_utc: expires,
        }
    }

    /// Repousse l'échéance du délai d'inactivité (sans effet si le délai est désactivé).
    ///
    /// Paramètres : aucun.
    pub fn touch(&self) {
        self.touch_in(&mut locked(&self.state));
    }

    /// Verrouille : génération +1, moteur arrêté.
    ///
    /// Paramètres : aucun.
    pub fn lock(&self) {
        Self::lock_in(&mut locked(&self.state));
    }

    /// Verrouille si le moteur est mort ou si le délai est échu.
    ///
    /// Paramètres : aucun.
    pub fn expire(&self) {
        self.expire_in(&mut locked(&self.state));
    }

    /// Point d'accès du moteur pour une opération.
    ///
    /// Paramètres : la génération relevée au début de l'opération.
    /// Retour : le moteur connecté.
    ///
    /// # Errors
    ///
    /// `locked` si la session est verrouillée ou si la génération a changé depuis.
    pub fn access(&self, generation: u64) -> Result<Arc<EngineSlot>> {
        let mut state = locked(&self.state);
        self.expire_in(&mut state);
        match &state.engine {
            Some(engine) if state.generation == generation => Ok(Arc::clone(engine)),
            _ => Err(vaultagent_core::ErrorCode::Locked.into()),
        }
    }

    /// Recalcule l'échéance dans un état déjà verrouillé.
    ///
    /// Paramètres : l'état.
    pub(crate) fn touch_in(&self, state: &mut State) {
        let minutes = self.store.account().and_then(|account| account.timeout_minutes);
        state.deadline = minutes.map(|value| self.clock.monotonic_ms() + u64::from(value) * 60_000);
    }

    /// Verrouille dans un état déjà verrouillé.
    ///
    /// Paramètres : l'état.
    pub(crate) fn lock_in(state: &mut State) {
        state.generation += 1;
        if let Some(engine) = state.engine.take() {
            engine.kill();
        }
        state.deadline = None;
    }

    /// Verrouille l'état si le moteur est mort ou l'échéance dépassée.
    ///
    /// Paramètres : l'état.
    fn expire_in(&self, state: &mut State) {
        let dead = state.engine.as_ref().is_some_and(|engine| !engine.alive());
        let late = state
            .deadline
            .is_some_and(|deadline| self.clock.monotonic_ms() >= deadline);
        if state.engine.is_some() && (dead || late) {
            Self::lock_in(state);
        }
    }
}

impl SessionControl for &Session {
    /// Verrouille si le délai est échu, puis indique si un moteur est connecté.
    fn unlocked(&mut self) -> bool {
        self.status().unlocked
    }

    /// Génération courante.
    fn generation(&self) -> u64 {
        Session::generation(self)
    }

    /// Profil du compte configuré.
    fn profile(&self) -> Option<String> {
        self.store.account().map(|account| account.profile)
    }

    /// Déverrouille avec le code facultatif.
    fn unlock(&mut self, code: Option<&str>) -> vaultagent_core::Result<()> {
        let code = code.map(|text| Zeroizing::new(text.to_owned()));
        Session::unlock(self, code.as_ref())
            .map(|_| ())
            .map_err(ServerError::into_code)
    }

    /// Verrouille la session.
    fn lock(&mut self) {
        Session::lock(self);
    }
}
