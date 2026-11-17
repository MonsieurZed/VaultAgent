//! Repli d'un texte pour comparer sans accents ni casse, sans dépendance : table des lettres accentuées courantes.

/// Lettres accentuées minuscules du français et de l'anglais, avec leur lettre de base.
const ACCENTED: [(&str, char); 8] = [
    ("àáâãäå", 'a'),
    ("ç", 'c'),
    ("èéêë", 'e'),
    ("ìíîï", 'i'),
    ("ñ", 'n'),
    ("òóôõö", 'o'),
    ("ùúûü", 'u'),
    ("ýÿ", 'y'),
];

/// Indique si un caractère est une marque d'accent combinante (U+0300 à U+036F).
///
/// Paramètres : le caractère.
/// Retour : vrai pour une marque combinante.
fn is_combining_mark(character: char) -> bool {
    ('\u{0300}'..='\u{036f}').contains(&character)
}

/// Remplace une lettre accentuée minuscule par sa lettre de base.
///
/// Paramètres : le caractère, déjà en minuscule.
/// Retour : la lettre de base, ou le caractère inchangé.
fn base_letter(character: char) -> char {
    ACCENTED
        .iter()
        .find(|(accented, _)| accented.contains(character))
        .map_or(character, |(_, base)| *base)
}

/// Replie un texte : minuscules, lettres accentuées ramenées à leur base, marques combinantes retirées.
///
/// Paramètres : le texte.
/// Retour : le texte replié, utilisé pour comparer un fragment de titre aux titres génériques.
#[must_use]
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .filter(|character| !is_combining_mark(*character))
        .map(base_letter)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accents_and_case_are_folded() {
        assert_eq!(fold("Nouvelle Fenêtre"), "nouvelle fenetre");
        assert_eq!(fold("ÉCOLE Çà Ñandú"), "ecole ca nandu");
        assert_eq!(fold("Über Ÿ"), "uber y");
        assert_eq!(fold("Garçon à Noël"), "garcon a noel");
    }

    #[test]
    fn decomposed_accents_are_folded_too() {
        assert_eq!(fold("e\u{0301}te\u{0301}"), "ete");
    }

    #[test]
    fn plain_text_is_only_lowercased() {
        assert_eq!(fold("about:blank 42"), "about:blank 42");
        assert_eq!(fold(""), "");
    }
}
