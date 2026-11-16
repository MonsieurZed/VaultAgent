//! Jeton d'appareil : texte secret effacé à la destruction, jamais affiché.

use std::fmt;

use vaultagent_core::device_token::parse_bearer;
use zeroize::Zeroizing;

use crate::error::ClientError;

/// Jeton `va1.<deviceId>.<secret>` ; son `Debug` ne montre rien du contenu.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(Zeroizing<String>);

impl Token {
    /// Valide la forme du jeton du protocole.
    ///
    /// Paramètres : le texte du jeton.
    /// Retour : le jeton, ou `ClientError::Config` si la forme est fausse (le texte n'est pas répété).
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn parse(text: &str) -> Result<Self, ClientError> {
        let header = Zeroizing::new(format!("Bearer {text}"));
        parse_bearer(Some(header.as_str()))
            .map(|_| Self(Zeroizing::new(text.to_owned())))
            .ok_or_else(|| ClientError::Config("Token has an invalid format.".to_owned()))
    }

    /// Valeur de l'en-tête `Authorization`, effacée à la destruction.
    #[must_use]
    pub fn bearer(&self) -> Zeroizing<String> {
        Zeroizing::new(format!("Bearer {}", self.0.as_str()))
    }

    /// Texte du jeton, réservé à l'écriture du fichier de configuration.
    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for Token {
    /// Écrit `Token(***)` sans aucun caractère du jeton.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Token(***)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "va1.0000000a-0000-4000-8000-00000000000b.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    #[test]
    fn accepts_the_protocol_shape() {
        let token = Token::parse(GOOD).unwrap();
        assert_eq!(token.expose(), GOOD);
        assert_eq!(token.bearer().as_str(), format!("Bearer {GOOD}"));
    }

    #[test]
    fn rejects_other_shapes_without_echo() {
        let error = Token::parse("not-a-token-secret").unwrap_err();
        assert!(!error.message().contains("not-a-token-secret"));
    }

    #[test]
    fn debug_hides_the_content() {
        let token = Token::parse(GOOD).unwrap();
        let shown = format!("{token:?}");
        assert!(!shown.contains("va1"));
        assert!(!shown.contains("AAAA"));
    }
}
