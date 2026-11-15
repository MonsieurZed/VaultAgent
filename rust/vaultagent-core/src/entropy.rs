//! Source d'aléa injectée : implémentation du système (`getrandom`) et implémentation déterministe pour les tests,
//! plus l'adaptateur vers les traits `rand_core` exigés par `rsa`.

use crate::error::{ErrorCode, Result};
use rand_core::{CryptoRng, Error as RngError, RngCore};
use sha2::{Digest, Sha256};

/// Source d'octets aléatoires ; la logique de la crate ne lit jamais d'aléa autrement.
pub trait Entropy {
    /// Remplit un tampon d'octets aléatoires.
    ///
    /// Paramètres : le tampon à remplir.
    /// Retour : rien.
    /// # Errors
    ///
    /// `engine-entropy` si la source est indisponible.
    fn fill(&mut self, buffer: &mut [u8]) -> Result<()>;

    /// Tire un entier uniforme dans `[0, bound)` par rejet, sans biais.
    ///
    /// Paramètres : la borne exclusive, comprise entre 1 et 2^32.
    /// Retour : l'entier tiré, ou 0 si la borne est nulle.
    /// # Errors
    ///
    /// `engine-entropy` si la source est indisponible.
    fn below(&mut self, bound: u32) -> Result<u32> {
        if bound == 0 {
            return Ok(0);
        }
        let space = u64::from(u32::MAX) + 1;
        let limit = space - space % u64::from(bound);
        loop {
            let mut bytes = [0u8; 4];
            self.fill(&mut bytes)?;
            let value = u64::from(u32::from_le_bytes(bytes));
            if value < limit {
                return Ok(u32::try_from(value % u64::from(bound)).unwrap_or(0));
            }
        }
    }
}

/// Code d'erreur rendu quand la source d'aléa échoue.
fn unavailable() -> ErrorCode {
    ErrorCode::Engine("engine-entropy".to_owned())
}

/// Aléa du système d'exploitation.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemEntropy;

impl Entropy for SystemEntropy {
    /// Remplit le tampon depuis le générateur du système.
    fn fill(&mut self, buffer: &mut [u8]) -> Result<()> {
        getrandom::getrandom(buffer).map_err(|_| unavailable())
    }
}

/// Aléa déterministe pour les tests : flux SHA-256 d'un compteur dérivé d'une graine. Jamais en production.
#[derive(Debug, Clone)]
pub struct SeededEntropy {
    seed: u64,
    block: u64,
    pending: Vec<u8>,
}

impl SeededEntropy {
    /// Crée un flux reproductible.
    ///
    /// Paramètres : la graine.
    /// Retour : la source.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            block: 0,
            pending: Vec::new(),
        }
    }
}

impl Entropy for SeededEntropy {
    /// Remplit le tampon avec la suite déterministe de la graine.
    fn fill(&mut self, buffer: &mut [u8]) -> Result<()> {
        for slot in buffer.iter_mut() {
            if self.pending.is_empty() {
                let mut hash = Sha256::new();
                hash.update(self.seed.to_le_bytes());
                hash.update(self.block.to_le_bytes());
                self.block += 1;
                self.pending = hash.finalize().to_vec();
                self.pending.reverse();
            }
            *slot = self.pending.pop().unwrap_or(0);
        }
        Ok(())
    }
}

/// Adaptateur d'une [`Entropy`] vers `RngCore` ; une panne est mémorisée et se lit par [`EntropyRng::finish`].
pub struct EntropyRng<'a, E: Entropy> {
    source: &'a mut E,
    failure: Option<ErrorCode>,
}

impl<'a, E: Entropy> EntropyRng<'a, E> {
    /// Enveloppe une source.
    ///
    /// Paramètres : la source à prêter.
    /// Retour : l'adaptateur.
    pub fn new(source: &'a mut E) -> Self {
        Self { source, failure: None }
    }

    /// Termine l'usage de l'adaptateur.
    ///
    /// Paramètres : aucun.
    /// Retour : rien.
    /// # Errors
    ///
    /// L'erreur de la source si elle a échoué pendant l'usage ; le résultat produit doit alors être jeté.
    pub fn finish(self) -> Result<()> {
        self.failure.map_or(Ok(()), Err)
    }
}

impl<E: Entropy> RngCore for EntropyRng<'_, E> {
    /// Tire 32 bits ; zéro si la source a échoué.
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    /// Tire 64 bits ; zéro si la source a échoué.
    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    /// Remplit le tampon ; en cas de panne il est mis à zéro et la panne est mémorisée.
    fn fill_bytes(&mut self, buffer: &mut [u8]) {
        if self.try_fill_bytes(buffer).is_err() {
            buffer.fill(0);
        }
    }

    /// Remplit le tampon et mémorise la première panne.
    fn try_fill_bytes(&mut self, buffer: &mut [u8]) -> std::result::Result<(), RngError> {
        match self.source.fill(buffer) {
            Ok(()) => Ok(()),
            Err(code) => {
                self.failure.get_or_insert(code);
                Err(RngError::from(std::num::NonZeroU32::MIN))
            }
        }
    }
}

impl<E: Entropy> CryptoRng for EntropyRng<'_, E> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_stream_is_reproducible_and_seed_dependent() {
        let (mut first, mut same_seed, mut other_seed) =
            (SeededEntropy::new(1), SeededEntropy::new(1), SeededEntropy::new(2));
        let (mut left, mut right, mut apart) = ([0u8; 70], [0u8; 70], [0u8; 70]);
        first.fill(&mut left).unwrap();
        same_seed.fill(&mut right).unwrap();
        other_seed.fill(&mut apart).unwrap();
        assert_eq!(left, right);
        assert_ne!(left, apart);
        assert!(left.iter().any(|byte| *byte != 0));
    }

    #[test]
    fn seeded_stream_does_not_depend_on_chunking() {
        let mut whole = SeededEntropy::new(9);
        let mut parts = SeededEntropy::new(9);
        let mut all = [0u8; 50];
        whole.fill(&mut all).unwrap();
        let (mut first, mut second) = ([0u8; 20], [0u8; 30]);
        parts.fill(&mut first).unwrap();
        parts.fill(&mut second).unwrap();
        assert_eq!([first.as_slice(), second.as_slice()].concat(), all);
    }

    #[test]
    fn below_stays_in_range_and_covers_it() {
        let mut entropy = SeededEntropy::new(3);
        let mut seen = [false; 32];
        for _ in 0..2000 {
            let value = entropy.below(32).unwrap();
            assert!(value < 32);
            seen[value as usize] = true;
        }
        assert!(seen.iter().all(|hit| *hit));
        assert_eq!(entropy.below(0).unwrap(), 0);
        assert_eq!(entropy.below(1).unwrap(), 0);
    }

    #[test]
    fn system_entropy_fills_with_varying_bytes() {
        let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
        SystemEntropy.fill(&mut a).unwrap();
        SystemEntropy.fill(&mut b).unwrap();
        assert_ne!(a, b);
    }

    struct Broken;

    impl Entropy for Broken {
        /// Échoue toujours, pour simuler une source d'aléa en panne.
        fn fill(&mut self, _buffer: &mut [u8]) -> Result<()> {
            Err(unavailable())
        }
    }

    #[test]
    fn adapter_reports_source_failure_through_finish() {
        let mut broken = Broken;
        let mut rng = EntropyRng::new(&mut broken);
        let mut buffer = [7u8; 8];
        rng.fill_bytes(&mut buffer);
        assert_eq!(buffer, [0u8; 8]);
        assert_eq!(rng.finish(), Err(ErrorCode::Engine("engine-entropy".into())));
        let mut seeded = SeededEntropy::new(1);
        let mut ok = EntropyRng::new(&mut seeded);
        ok.next_u64();
        assert_eq!(ok.finish(), Ok(()));
    }
}
