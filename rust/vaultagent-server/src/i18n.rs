//! Traductions du front : les textes sont indexés par leur version française, comme dans l'ancien `app.js`.
//! Un texte absent des tables est rendu tel quel (français).

use crate::front_escape::escape;
use crate::front_lang::Lang;
use crate::{i18n_account, i18n_devices, i18n_errors, i18n_help, i18n_layout};

/// Tables de traduction, chacune dans son fichier.
const TABLES: [&[(&str, &str, &str)]; 5] = [
    i18n_layout::ENTRIES,
    i18n_account::ENTRIES,
    i18n_devices::ENTRIES,
    i18n_help::ENTRIES,
    i18n_errors::ENTRIES,
];

/// Traduit un texte français.
///
/// Paramètres : la langue, le texte français qui sert de clé.
/// Retour : le texte dans la langue demandée, ou le texte français s'il n'est pas dans les tables.
#[must_use]
pub fn translate(lang: Lang, source: &'static str) -> &'static str {
    if lang == Lang::Fr {
        return source;
    }
    TABLES
        .iter()
        .flat_map(|table| table.iter())
        .find(|(french, _, _)| *french == source)
        .map_or(
            source,
            |(_, english, spanish)| if lang == Lang::En { english } else { spanish },
        )
}

/// Traduit un texte français et l'échappe pour l'insérer dans une page.
///
/// Paramètres : la langue, le texte français qui sert de clé.
/// Retour : le texte traduit, échappé en HTML.
#[must_use]
pub fn text(lang: Lang, source: &'static str) -> String {
    escape(translate(lang, source))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_follow_the_old_front_in_three_languages() {
        assert_eq!(translate(Lang::Fr, "Verrouiller"), "Verrouiller");
        assert_eq!(translate(Lang::En, "Verrouiller"), "Lock");
        assert_eq!(translate(Lang::Es, "Verrouiller"), "Bloquear");
        assert_eq!(translate(Lang::En, "Appareils"), "Devices");
    }

    #[test]
    fn an_unknown_text_stays_french() {
        assert_eq!(translate(Lang::En, "Texte inconnu"), "Texte inconnu");
    }

    #[test]
    fn every_entry_is_complete_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for (french, english, spanish) in TABLES.iter().flat_map(|table| table.iter()) {
            assert!(!french.is_empty() && !english.is_empty() && !spanish.is_empty());
            assert!(seen.insert(*french), "doublon : {french}");
        }
        assert_eq!(seen.len(), 89);
    }
}
