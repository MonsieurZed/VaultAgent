//! Attente injectée : les tests n'attendent jamais pour de vrai.

use std::time::Duration;

/// Source d'attente entre deux interrogations.
pub trait Pause {
    /// Attend la durée donnée.
    ///
    /// Paramètres : la durée.
    fn pause(&mut self, duration: Duration);
}

/// Attente réelle du binaire.
pub struct ThreadPause;

impl Pause for ThreadPause {
    /// Endort le fil courant.
    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Attente factice : note les durées sans attendre.
#[derive(Default)]
pub struct RecordedPause {
    /// Durées demandées, dans l'ordre.
    pub durations: Vec<Duration>,
}

impl Pause for RecordedPause {
    /// Note la durée et rend aussitôt la main.
    fn pause(&mut self, duration: Duration) {
        self.durations.push(duration);
    }
}
