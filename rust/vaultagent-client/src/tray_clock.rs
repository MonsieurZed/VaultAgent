//! Heure courante de l'application de barre, seule lecture d'horloge de l'interface.

/// Lit l'heure du système.
///
/// Retour : les secondes écoulées depuis 1970, 0 si l'horloge est avant 1970.
#[must_use]
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_is_after_2026() {
        assert!(now_unix() > 1_767_225_600);
    }
}
