//! Messages d'état de la page : un code court dans l'URL (`msg=`), traduit au rendu, jamais un texte libre.

/// Message affiché après une action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// Action réussie.
    Saved,
    /// Action refusée sans précision.
    Refused,
    /// Connexion au coffre refusée.
    LoginRefused,
    /// Compte déjà configuré dans un autre coffre.
    DuplicateAccount,
    /// Demande expirée.
    Expired,
}

impl Notice {
    /// Lit la valeur du paramètre `msg`.
    ///
    /// Paramètres : le code.
    /// Retour : le message, ou `None` pour un code inconnu.
    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "saved" => Some(Self::Saved),
            "refused" => Some(Self::Refused),
            "login-refused" => Some(Self::LoginRefused),
            "duplicate-account" => Some(Self::DuplicateAccount),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }

    /// Valeur du paramètre `msg`.
    ///
    /// Paramètres : aucun.
    /// Retour : le code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::Refused => "refused",
            Self::LoginRefused => "login-refused",
            Self::DuplicateAccount => "duplicate-account",
            Self::Expired => "expired",
        }
    }

    /// Texte français, clé de traduction.
    ///
    /// Paramètres : aucun.
    /// Retour : le texte.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Saved => "Enregistré.",
            Self::Refused => "Opération refusée.",
            Self::LoginRefused => {
                "Connexion refusée. Vérifiez le mot de passe maître du coffre et le code 2FA éventuel."
            }
            Self::DuplicateAccount => "Ce compte est déjà configuré dans un autre coffre.",
            Self::Expired => "Cette demande a expiré.",
        }
    }

    /// Message d'une erreur du protocole : `login:*` et `unlock:*` donnent la connexion refusée.
    ///
    /// Paramètres : le code d'erreur de la réponse JSON.
    /// Retour : le message à afficher.
    #[must_use]
    pub fn from_error(code: &str) -> Self {
        if code == "duplicate-account" {
            Self::DuplicateAccount
        } else if code == "request-expired" {
            Self::Expired
        } else if code.starts_with("login:") || code.starts_with("unlock:") {
            Self::LoginRefused
        } else {
            Self::Refused
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_errors_map_to_codes() {
        assert_eq!(Notice::from_error("login:credentials"), Notice::LoginRefused);
        assert_eq!(Notice::from_error("unlock:two-factor"), Notice::LoginRefused);
        assert_eq!(Notice::from_error("duplicate-account"), Notice::DuplicateAccount);
        assert_eq!(Notice::from_error("request-expired"), Notice::Expired);
        assert_eq!(Notice::from_error("invalid-account"), Notice::Refused);
    }

    #[test]
    fn codes_round_trip() {
        for notice in [
            Notice::Saved,
            Notice::Refused,
            Notice::LoginRefused,
            Notice::DuplicateAccount,
            Notice::Expired,
        ] {
            assert_eq!(Notice::parse(notice.code()), Some(notice));
        }
        assert_eq!(Notice::parse("<script>"), None);
    }
}
