//! Écrans de la page : la page principale `/` et les pages secondaires `/?view=vault|add|help`.

/// Écran affiché.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Page principale : barre d'état, coffre, demandes et appareils.
    Main,
    /// Formulaire du coffre.
    Vault,
    /// Ajout d'un appareil.
    Add,
    /// Aide.
    Help,
}

impl Screen {
    /// Lit la valeur du paramètre `view`.
    ///
    /// Paramètres : `vault`, `add` ou `help`.
    /// Retour : la page secondaire, ou `None` pour tout autre texte (la page principale s'affiche alors).
    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        [Self::Vault, Self::Add, Self::Help]
            .into_iter()
            .find(|screen| screen.code() == Some(code))
    }

    /// Valeur du paramètre `view`.
    ///
    /// Paramètres : aucun.
    /// Retour : le code d'une page secondaire, `None` pour la page principale.
    #[must_use]
    pub fn code(self) -> Option<&'static str> {
        match self {
            Self::Main => None,
            Self::Vault => Some("vault"),
            Self::Add => Some("add"),
            Self::Help => Some("help"),
        }
    }

    /// Adresse de l'écran, avec des paramètres supplémentaires déjà sûrs (codes).
    ///
    /// Paramètres : les paires nom-valeur à ajouter après `view`.
    /// Retour : `/` seul, ou `/?view=…&nom=valeur`.
    #[must_use]
    pub fn href(self, params: &[(&str, &str)]) -> String {
        let pairs: Vec<String> = self
            .code()
            .map(|code| format!("view={code}"))
            .into_iter()
            .chain(params.iter().map(|(name, value)| format!("{name}={value}")))
            .collect();
        if pairs.is_empty() {
            "/".to_owned()
        } else {
            format!("/?{}", pairs.join("&"))
        }
    }

    /// Adresse de l'écran pour un attribut HTML (`&` devient `&amp;`).
    ///
    /// Paramètres : comme [`Screen::href`].
    /// Retour : l'adresse échappée.
    #[must_use]
    pub fn href_html(self, params: &[(&str, &str)]) -> String {
        self.href(params).replace('&', "&amp;")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondary_codes_round_trip_and_anything_else_is_not_a_screen() {
        for screen in [Screen::Vault, Screen::Add, Screen::Help] {
            assert_eq!(Screen::parse(screen.code().unwrap()), Some(screen));
        }
        assert_eq!(Screen::parse("account"), None);
        assert_eq!(Screen::parse(""), None);
    }

    #[test]
    fn addresses_omit_the_view_for_the_main_page() {
        assert_eq!(Screen::Main.href(&[]), "/");
        assert_eq!(Screen::Main.href(&[("msg", "saved")]), "/?msg=saved");
        assert_eq!(Screen::Vault.href(&[("msg", "saved")]), "/?view=vault&msg=saved");
        assert_eq!(Screen::Add.href_html(&[("lang", "en")]), "/?view=add&amp;lang=en");
    }
}
