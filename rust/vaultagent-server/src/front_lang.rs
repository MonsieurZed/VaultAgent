//! Langue du front : français par défaut, choisie par `?lang=` puis mémorisée dans le cookie `vaultagent-language`.

/// Nom du cookie de langue.
pub const COOKIE: &str = "vaultagent-language";

/// Langue d'affichage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// Français (par défaut).
    Fr,
    /// Anglais.
    En,
    /// Espagnol.
    Es,
}

impl Lang {
    /// Les trois langues, dans l'ordre du sélecteur.
    pub const ALL: [Lang; 3] = [Lang::Fr, Lang::En, Lang::Es];

    /// Lit un code de langue.
    ///
    /// Paramètres : `fr`, `en` ou `es`.
    /// Retour : la langue, ou `None` pour tout autre texte.
    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "fr" => Some(Self::Fr),
            "en" => Some(Self::En),
            "es" => Some(Self::Es),
            _ => None,
        }
    }

    /// Code à deux lettres.
    ///
    /// Paramètres : aucun.
    /// Retour : `fr`, `en` ou `es`.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Fr => "fr",
            Self::En => "en",
            Self::Es => "es",
        }
    }

    /// Nom de la langue dans sa propre langue.
    ///
    /// Paramètres : aucun.
    /// Retour : le nom affiché dans la liste de choix.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Fr => "Français",
            Self::En => "English",
            Self::Es => "Español",
        }
    }

    /// Langue demandée : le paramètre d'abord, puis le cookie, sinon le français.
    ///
    /// Paramètres : la valeur du paramètre `lang`, l'en-tête `cookie`.
    /// Retour : la langue.
    #[must_use]
    pub fn choose(requested: Option<&str>, cookie_header: Option<&str>) -> Self {
        requested
            .and_then(Self::parse)
            .or_else(|| cookie_header.and_then(from_cookie))
            .unwrap_or(Self::Fr)
    }

    /// Valeur de l'en-tête `Set-Cookie` qui mémorise la langue.
    ///
    /// Paramètres : aucun.
    /// Retour : la valeur complète avec ses attributs.
    #[must_use]
    pub fn set_cookie(self) -> String {
        format!(
            "{COOKIE}={}; Path=/; SameSite=Strict; HttpOnly; Secure; Max-Age=31536000",
            self.code()
        )
    }
}

/// Langue du cookie `vaultagent-language` dans un en-tête `Cookie`.
fn from_cookie(header: &str) -> Option<Lang> {
    header
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .and_then(|(_, value)| Lang::parse(value.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parameter_wins_then_the_cookie_then_french() {
        assert_eq!(Lang::choose(Some("en"), Some("vaultagent-language=es")), Lang::En);
        assert_eq!(Lang::choose(None, Some("a=1; vaultagent-language=es; b=2")), Lang::Es);
        assert_eq!(Lang::choose(Some("de"), Some("vaultagent-language=xx")), Lang::Fr);
        assert_eq!(Lang::choose(None, None), Lang::Fr);
    }

    #[test]
    fn the_cookie_is_strict_http_only_and_secure() {
        let cookie = Lang::En.set_cookie();
        for part in [
            "vaultagent-language=en",
            "Path=/",
            "SameSite=Strict",
            "HttpOnly",
            "Secure",
        ] {
            assert!(cookie.contains(part), "{part}");
        }
    }
}
