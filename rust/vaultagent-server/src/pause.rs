//! Attente injectée : les boucles de relecture et l'attente du TOTP dorment par ce trait, jamais directement.

use std::sync::Mutex;
use std::time::Duration;

use crate::sync::locked;

/// Source d'attente d'un fil.
pub trait Pause: Send + Sync {
    /// Attend la durée donnée.
    ///
    /// Paramètres : la durée en millisecondes.
    fn sleep_ms(&self, milliseconds: u64);
}

/// Attente réelle du système.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThreadPause;

impl Pause for ThreadPause {
    /// Endort le fil courant.
    fn sleep_ms(&self, milliseconds: u64) {
        std::thread::sleep(Duration::from_millis(milliseconds));
    }
}

/// Attente factice des tests : ne dort pas, garde la liste des durées demandées.
#[derive(Debug, Default)]
pub struct RecordedPause(Mutex<Vec<u64>>);

impl RecordedPause {
    /// Crée une attente factice vide.
    ///
    /// Paramètres : aucun.
    /// Retour : l'attente.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Durées demandées jusqu'ici, dans l'ordre.
    ///
    /// Paramètres : aucun.
    /// Retour : les millisecondes.
    #[must_use]
    pub fn waits(&self) -> Vec<u64> {
        locked(&self.0).clone()
    }
}

impl Pause for RecordedPause {
    /// Note la durée sans dormir.
    fn sleep_ms(&self, milliseconds: u64) {
        locked(&self.0).push(milliseconds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_pause_keeps_the_waits() {
        let pause = RecordedPause::new();
        pause.sleep_ms(200);
        pause.sleep_ms(5);
        assert_eq!(pause.waits(), vec![200, 5]);
    }
}
