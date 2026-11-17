//! Traduction de l'interface de barre : textes français du client .NET (`translations.json`, copié dans le dépôt Rust
//! et inclus à la compilation), langue choisie par la préférence du registre puis par celle de Windows.

use std::collections::HashMap;

const TRANSLATIONS: &str = include_str!("../translations.json");

/// Langue de l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// Français, langue des clés.
    French,
    /// Anglais.
    English,
    /// Espagnol.
    Spanish,
}

impl Language {
    /// Lit un code de langue.
    ///
    /// Paramètres : un code comme `fr`, `en`, `es` ou un nom de langue Windows comme `fr-FR`.
    /// Retour : la langue connue, ou `None`.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        let prefix = code.trim().get(..2)?.to_ascii_lowercase();
        match prefix.as_str() {
            "fr" => Some(Self::French),
            "en" => Some(Self::English),
            "es" => Some(Self::Spanish),
            _ => None,
        }
    }

    /// Choisit la langue de l'interface.
    ///
    /// Paramètres : la préférence enregistrée (registre) et le nom de langue de Windows.
    /// Retour : la préférence si elle est valide, sinon la langue de Windows si elle est connue, sinon l'anglais.
    #[must_use]
    pub fn choose(preference: Option<&str>, windows_locale: &str) -> Self {
        preference
            .and_then(Self::from_code)
            .or_else(|| Self::from_code(windows_locale))
            .unwrap_or(Self::English)
    }
}

/// Traducteur d'une langue.
#[derive(Debug, Clone)]
pub struct Translator {
    language: Language,
    table: HashMap<String, Vec<String>>,
}

impl Translator {
    /// Construit le traducteur à partir du fichier inclus.
    ///
    /// Paramètres : la langue.
    /// Retour : le traducteur ; une table illisible donne un traducteur qui rend les clés françaises.
    #[must_use]
    pub fn new(language: Language) -> Self {
        Self {
            language,
            table: serde_json::from_str(TRANSLATIONS).unwrap_or_default(),
        }
    }

    /// Traduit un texte français.
    ///
    /// Paramètres : le texte français (la clé).
    /// Retour : la traduction ; le texte lui-même en français ou si la clé est inconnue.
    #[must_use]
    pub fn t(&self, key: &str) -> String {
        let index = match self.language {
            Language::French => return key.to_owned(),
            Language::English => 0,
            Language::Spanish => 1,
        };
        self.table
            .get(key)
            .and_then(|values| values.get(index))
            .cloned()
            .unwrap_or_else(|| key.to_owned())
    }

    /// Traduit un modèle puis remplace ses champs `{nom}`.
    ///
    /// Paramètres : le modèle français et les couples (nom, valeur).
    /// Retour : le texte traduit rempli.
    #[must_use]
    pub fn fill(&self, key: &str, fields: &[(&str, &str)]) -> String {
        fields.iter().fold(self.t(key), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_is_chosen_from_preference_then_windows() {
        assert_eq!(Language::choose(Some("es"), "fr-FR"), Language::Spanish);
        assert_eq!(Language::choose(Some("zz"), "fr-FR"), Language::French);
        assert_eq!(Language::choose(None, "es-MX"), Language::Spanish);
        assert_eq!(Language::choose(None, "de-DE"), Language::English);
        assert_eq!(Language::choose(None, ""), Language::English);
        assert_eq!(Language::from_code("é"), None);
    }

    #[test]
    fn keys_translate_and_unknown_keys_pass_through() {
        let english = Translator::new(Language::English);
        assert_eq!(english.t("Quitter"), "Quit");
        assert_eq!(english.t("texte inconnu"), "texte inconnu");
        assert_eq!(Translator::new(Language::Spanish).t("Quitter"), "Salir");
        assert_eq!(Translator::new(Language::French).t("Quitter"), "Quitter");
    }

    #[test]
    fn every_entry_has_english_and_spanish() {
        let table: HashMap<String, Vec<String>> = serde_json::from_str(TRANSLATIONS).unwrap();
        assert!(table.len() > 100);
        assert!(table.values().all(|values| values.len() == 2));
    }

    #[test]
    fn templates_are_filled() {
        let text = Translator::new(Language::French).fill(
            "{client} demande l'accès à {vault}",
            &[("client", "PC"), ("vault", "Coffre")],
        );
        assert_eq!(text, "PC demande l'accès à Coffre");
        let english = Translator::new(Language::English).fill(
            "{client} demande l'accès à {vault}",
            &[("client", "PC"), ("vault", "Vault")],
        );
        assert_eq!(english, "PC requests access to Vault");
    }
}
