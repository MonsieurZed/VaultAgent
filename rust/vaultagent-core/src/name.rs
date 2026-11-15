//! Noms de fiche et de dossier : non vides, bornés en unités UTF-16, sans caractère de contrôle.

use crate::jsval::{has_control, is_blank, utf16_len};

/// Longueur maximale d'un nom de fiche.
pub const ITEM_NAME_MAX: usize = 500;

/// Longueur maximale d'un nom de dossier.
pub const FOLDER_NAME_MAX: usize = 250;

/// Nom validé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedName(String);

impl BoundedName {
    /// Valide un nom.
    ///
    /// Paramètres : le texte et la longueur maximale en unités UTF-16.
    /// Retour : le nom, ou `None` s'il est vide ou blanc, trop long, ou contient un caractère de contrôle.
    #[must_use]
    pub fn parse(text: &str, max: usize) -> Option<Self> {
        let valid = !is_blank(text) && utf16_len(text) <= max && !has_control(text);
        valid.then(|| Self(text.to_owned()))
    }

    /// Texte du nom.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bounded_names() {
        assert!(BoundedName::parse("Compte factice", ITEM_NAME_MAX).is_some());
        assert!(BoundedName::parse(&"a".repeat(250), FOLDER_NAME_MAX).is_some());
    }

    #[test]
    fn refuses_blank_long_and_control() {
        assert!(BoundedName::parse("", 10).is_none());
        assert!(BoundedName::parse("  \t", 10).is_none());
        assert!(BoundedName::parse(&"a".repeat(251), FOLDER_NAME_MAX).is_none());
        assert!(BoundedName::parse("a\nb", 10).is_none());
        assert!(BoundedName::parse(&"\u{1f600}".repeat(126), FOLDER_NAME_MAX).is_none());
    }
}
