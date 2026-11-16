//! Horloge injectée : temps monotone en millisecondes pour les délais, heure murale pour `expiresUtc`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Source de temps de la session.
pub trait Clock: Send + Sync {
    /// Millisecondes écoulées sur une horloge monotone (origine arbitraire).
    fn monotonic_ms(&self) -> u64;

    /// Millisecondes depuis 1970 (UTC).
    fn wall_ms(&self) -> u64;
}

/// Horloge du système.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    start: Instant,
}

impl SystemClock {
    /// Crée l'horloge ; l'origine monotone est l'instant de création.
    ///
    /// Paramètres : aucun.
    /// Retour : l'horloge.
    #[must_use]
    pub fn new() -> Self {
        Self { start: Instant::now() }
    }
}

impl Default for SystemClock {
    /// Équivaut à [`SystemClock::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    /// Temps écoulé depuis la création.
    fn monotonic_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Heure du système, 0 si elle précède 1970.
    fn wall_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
    }
}

/// Horloge manuelle pour les tests : le temps n'avance que par [`ManualClock::advance`].
#[derive(Debug)]
pub struct ManualClock {
    elapsed: AtomicU64,
    wall_base: u64,
}

impl ManualClock {
    /// Crée une horloge à zéro.
    ///
    /// Paramètres : l'heure murale (ms Unix) correspondant au temps monotone zéro.
    /// Retour : l'horloge.
    #[must_use]
    pub fn new(wall_base: u64) -> Self {
        Self {
            elapsed: AtomicU64::new(0),
            wall_base,
        }
    }

    /// Fait avancer le temps.
    ///
    /// Paramètres : les millisecondes à ajouter.
    pub fn advance(&self, milliseconds: u64) {
        self.elapsed.fetch_add(milliseconds, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    /// Temps cumulé par `advance`.
    fn monotonic_ms(&self) -> u64 {
        self.elapsed.load(Ordering::SeqCst)
    }

    /// Base murale plus temps cumulé.
    fn wall_ms(&self) -> u64 {
        self.wall_base + self.monotonic_ms()
    }
}
