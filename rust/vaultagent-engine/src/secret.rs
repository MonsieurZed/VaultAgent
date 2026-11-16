//! Texte secret (mot de passe, TOTP, notes, clé de passkey) : effacé à la destruction, jamais écrit par `Debug`.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

/// Secret en clair, effacé de la mémoire à sa destruction.
#[derive(Clone)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    /// Enveloppe un texte secret.
    ///
    /// Paramètres : le texte, déplacé dans le secret.
    /// Retour : le secret.
    #[must_use]
    pub fn new(text: String) -> Self {
        Self(Zeroizing::new(text))
    }

    /// Texte en clair, à ne pas conserver plus que nécessaire.
    ///
    /// Paramètres : aucun.
    /// Retour : le texte.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Secret {
    /// Copie un texte dans un secret.
    fn from(text: &str) -> Self {
        Self::new(text.to_owned())
    }
}

impl PartialEq for Secret {
    /// Compare les textes en clair (usage des tests et de la vérification par relecture).
    fn eq(&self, other: &Self) -> bool {
        self.expose() == other.expose()
    }
}

impl Eq for Secret {}

impl fmt::Debug for Secret {
    /// N'écrit que la longueur, jamais le contenu.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret({} octets)", self.expose().len())
    }
}

impl Serialize for Secret {
    /// Sérialise comme une chaîne ordinaire.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for Secret {
    /// Lit une chaîne ordinaire.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_content_and_serde_round_trips() {
        let secret = Secret::from("fake-password");
        assert!(!format!("{secret:?}").contains("fake-password"));
        let text = serde_json::to_string(&secret).unwrap_or_default();
        assert_eq!(text, "\"fake-password\"");
        assert_eq!(serde_json::from_str::<Secret>(&text).ok(), Some(secret));
    }
}
