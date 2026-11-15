//! Identifiant UUID validé comme celui de Node (`write-checks.mjs`), casse conservée.

use serde::{Serialize, Serializer};
use std::fmt;

/// UUID textuel `8-4-4-4-12` en hexadécimal, majuscules et minuscules admises, texte d'origine conservé.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Uuid(String);

impl Uuid {
    /// Valide un texte.
    ///
    /// Paramètres : le texte candidat.
    /// Retour : l'identifiant, ou `None` si la forme n'est pas `8-4-4-4-12` caractères hexadécimaux.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let valid = bytes.len() == 36
            && bytes.iter().enumerate().all(|(index, byte)| match index {
                8 | 13 | 18 | 23 => *byte == b'-',
                _ => byte.is_ascii_hexdigit(),
            });
        valid.then(|| Self(text.to_owned()))
    }

    /// Texte de l'identifiant, tel que fourni.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Uuid {
    /// Écrit l'identifiant sous la forme `Uuid(texte)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Uuid({})", self.0)
    }
}

impl fmt::Display for Uuid {
    /// Écrit le texte de l'identifiant.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for Uuid {
    /// Sérialise l'identifiant comme une chaîne.
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_both_cases_and_keeps_text() {
        let upper = "ABCDEF00-0000-4000-8000-00000000000A";
        assert_eq!(Uuid::parse(upper).map(|u| u.to_string()), Some(upper.to_owned()));
        assert!(Uuid::parse("00000000-0000-4000-8000-000000000001").is_some());
    }

    #[test]
    fn refuses_malformed_text() {
        for bad in [
            "",
            "pas-un-uuid",
            "00000000-0000-4000-8000-00000000000",
            "00000000-0000-4000-8000-0000000000012",
            "0000000000004000800000000000000-0001",
            "g0000000-0000-4000-8000-000000000001",
            "00000000-0000-4000-8000-00000000000é",
        ] {
            assert!(Uuid::parse(bad).is_none(), "{bad}");
        }
    }
}
