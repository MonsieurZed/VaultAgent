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
    /// Page périmée : le contrôle d'origine a échoué.
    StalePage,
    /// Corps du formulaire trop gros.
    TooLarge,
    /// Action inconnue.
    UnknownAction,
    /// Requête invalide (type de corps, méthode).
    BadRequest,
    /// Coffre verrouillé.
    Locked,
    /// Coffre absent ou non configuré.
    NotConfigured,
    /// Valeur saisie invalide.
    InvalidInput,
    /// Trop de demandes ou limite atteinte.
    TooMany,
}

const ALL: [Notice; 13] = [
    Notice::Saved,
    Notice::Refused,
    Notice::LoginRefused,
    Notice::DuplicateAccount,
    Notice::Expired,
    Notice::StalePage,
    Notice::TooLarge,
    Notice::UnknownAction,
    Notice::BadRequest,
    Notice::Locked,
    Notice::NotConfigured,
    Notice::InvalidInput,
    Notice::TooMany,
];

impl Notice {
    /// Lit la valeur du paramètre `msg`.
    ///
    /// Paramètres : le code.
    /// Retour : le message, ou `None` pour un code inconnu.
    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        ALL.into_iter().find(|notice| notice.code() == code)
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
            Self::StalePage => "stale-page",
            Self::TooLarge => "too-large",
            Self::UnknownAction => "unknown-action",
            Self::BadRequest => "bad-request",
            Self::Locked => "locked",
            Self::NotConfigured => "not-configured",
            Self::InvalidInput => "invalid-input",
            Self::TooMany => "too-many",
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
            Self::Refused => "Action refusée par le serveur.",
            Self::LoginRefused => {
                "Connexion refusée. Vérifiez le mot de passe maître du coffre et le code 2FA éventuel."
            }
            Self::DuplicateAccount => "Ce compte est déjà configuré dans un autre coffre.",
            Self::Expired => "Cette demande a expiré.",
            Self::StalePage => "La page n’était plus à jour, l’action n’a pas été faite. Réessaie.",
            Self::TooLarge => "Les données envoyées sont trop grosses, l’action n’a pas été faite.",
            Self::UnknownAction => "Action inconnue, rien n’a été fait.",
            Self::BadRequest => "Requête invalide, l’action n’a pas été faite.",
            Self::Locked => "Le coffre est verrouillé. Déverrouille-le d’abord.",
            Self::NotConfigured => "Le coffre n’est pas configuré.",
            Self::InvalidInput => "Une valeur saisie est invalide.",
            Self::TooMany => "Trop de demandes ou limite atteinte. Réessaie plus tard.",
        }
    }

    /// Message d'une erreur du protocole ; un code inconnu donne `Refused`.
    ///
    /// Paramètres : le code d'erreur de la réponse JSON (`login:*` et `unlock:*` donnent la connexion refusée).
    /// Retour : le message à afficher.
    #[must_use]
    pub fn from_error(code: &str) -> Self {
        match code {
            "origin-refused" => Self::StalePage,
            "request-size" => Self::TooLarge,
            "route-refused" => Self::UnknownAction,
            "request-refused" | "json-required" | "method-refused" => Self::BadRequest,
            "locked" => Self::Locked,
            "vault-required" | "vault-missing" | "not-configured" => Self::NotConfigured,
            "invalid-account" | "invalid-timeout" | "password-required" | "invalid-field" | "field-missing"
            | "invalid-device" | "invalid-key" | "invalid-vault-name" => Self::InvalidInput,
            "pairing-rate-limit" | "pairing-limit" | "device-limit" => Self::TooMany,
            "duplicate-account" => Self::DuplicateAccount,
            "request-expired" => Self::Expired,
            _ if code.starts_with("login:") || code.starts_with("unlock:") => Self::LoginRefused,
            _ => Self::Refused,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_protocol_error_has_its_notice() {
        let table = [
            ("origin-refused", Notice::StalePage),
            ("request-size", Notice::TooLarge),
            ("route-refused", Notice::UnknownAction),
            ("request-refused", Notice::BadRequest),
            ("json-required", Notice::BadRequest),
            ("method-refused", Notice::BadRequest),
            ("locked", Notice::Locked),
            ("vault-required", Notice::NotConfigured),
            ("vault-missing", Notice::NotConfigured),
            ("not-configured", Notice::NotConfigured),
            ("invalid-account", Notice::InvalidInput),
            ("invalid-timeout", Notice::InvalidInput),
            ("password-required", Notice::InvalidInput),
            ("invalid-field", Notice::InvalidInput),
            ("field-missing", Notice::InvalidInput),
            ("invalid-device", Notice::InvalidInput),
            ("invalid-key", Notice::InvalidInput),
            ("invalid-vault-name", Notice::InvalidInput),
            ("pairing-rate-limit", Notice::TooMany),
            ("pairing-limit", Notice::TooMany),
            ("device-limit", Notice::TooMany),
            ("duplicate-account", Notice::DuplicateAccount),
            ("request-expired", Notice::Expired),
            ("login:credentials", Notice::LoginRefused),
            ("unlock:two-factor", Notice::LoginRefused),
            ("owner-required", Notice::Refused),
            ("cancelled", Notice::Refused),
            ("something-new", Notice::Refused),
        ];
        for (code, notice) in table {
            assert_eq!(Notice::from_error(code), notice, "{code}");
        }
    }

    #[test]
    fn an_unknown_code_reads_as_a_server_refusal() {
        assert_eq!(Notice::from_error("zzz").label(), "Action refusée par le serveur.");
        assert!(Notice::StalePage.label().starts_with("La page n’était plus à jour"));
    }

    #[test]
    fn codes_round_trip() {
        for notice in ALL {
            assert_eq!(Notice::parse(notice.code()), Some(notice));
        }
        assert_eq!(Notice::parse("<script>"), None);
    }
}
