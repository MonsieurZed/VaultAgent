//! Onglets de la page : liens `?tab=account|clients|about`, l'onglet actif est seul rendu.

/// Onglet de la page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Formulaire du compte.
    Account,
    /// Clients, demandes et appareils.
    Clients,
    /// Page « Fonctionnement ».
    About,
}

impl Tab {
    /// Les onglets dans l'ordre d'affichage.
    pub const ALL: [Tab; 3] = [Tab::Account, Tab::Clients, Tab::About];

    /// Lit la valeur du paramètre `tab`.
    ///
    /// Paramètres : `account`, `clients` ou `about`.
    /// Retour : l'onglet, ou `None` pour tout autre texte.
    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.code() == code)
    }

    /// Valeur du paramètre `tab`.
    ///
    /// Paramètres : aucun.
    /// Retour : `account`, `clients` ou `about`.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Clients => "clients",
            Self::About => "about",
        }
    }

    /// Libellé français, clé de traduction.
    ///
    /// Paramètres : aucun.
    /// Retour : le texte de l'onglet.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Account => "Compte",
            Self::Clients => "Clients",
            Self::About => "Fonctionnement",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_unknown_is_refused() {
        for tab in Tab::ALL {
            assert_eq!(Tab::parse(tab.code()), Some(tab));
        }
        assert_eq!(Tab::parse("settings"), None);
    }
}
