//! Codes de sortie stables du binaire `vaultagent`.

/// Code de sortie du processus, fixé par le plan de la phase 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// Succès.
    Success,
    /// Usage ou donnée invalide.
    Usage,
    /// Refus du serveur.
    Refused,
    /// Cible refusée (fenêtre), utilisé par la saisie de la phase 4.
    TargetRefused,
    /// Coffre verrouillé (HTTP 423).
    Locked,
    /// Résultat indéterminé : délai ou coupure après l'envoi.
    Indeterminate,
}

impl ExitCode {
    /// Valeur numérique rendue au système.
    ///
    /// Retour : 0 succès, 1 usage, 2 refus, 3 cible refusée, 4 verrouillé, 5 indéterminé.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Usage => 1,
            Self::Refused => 2,
            Self::TargetRefused => 3,
            Self::Locked => 4,
            Self::Indeterminate => 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_follow_the_plan() {
        let all = [
            (ExitCode::Success, 0),
            (ExitCode::Usage, 1),
            (ExitCode::Refused, 2),
            (ExitCode::TargetRefused, 3),
            (ExitCode::Locked, 4),
            (ExitCode::Indeterminate, 5),
        ];
        for (exit, value) in all {
            assert_eq!(exit.code(), value);
        }
    }
}
