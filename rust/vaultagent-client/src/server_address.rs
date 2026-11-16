//! Adresse du serveur `VaultAgent` : origine HTTPS sans identifiants, chemin, requête ni fragment.

use vaultagent_core::{HttpsAddress, https_origin};

use crate::error::ClientError;

/// Origine HTTPS validée du serveur (`https://hôte[:port]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerAddress(String);

impl ServerAddress {
    /// Valide une adresse comme `validServer` de Node : `https` seulement, sans identifiants, chemin `/` ou vide,
    /// ni requête ni fragment.
    ///
    /// Paramètres : le texte saisi ou lu dans la configuration.
    /// Retour : l'adresse normalisée en origine, ou `ClientError::Usage` si elle ne convient pas.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn parse(text: &str) -> Result<Self, ClientError> {
        let refused = || ClientError::Usage("HTTPS VaultAgent server address required.".to_owned());
        HttpsAddress::parse(text).ok_or_else(refused)?;
        let origin = https_origin(text).ok_or_else(refused)?;
        let after_scheme = text.split_once("//").map_or("", |(_, rest)| rest);
        let path = after_scheme.find('/').map_or("", |index| &after_scheme[index..]);
        matches!(path, "" | "/").then_some(Self(origin)).ok_or_else(refused)
    }

    /// Origine normalisée, sans barre finale.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// URL complète d'un chemin du serveur.
    ///
    /// Paramètres : le chemin commençant par `/`.
    /// Retour : l'URL absolue.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_plain_origin() {
        assert_eq!(
            ServerAddress::parse("https://Vault.example.test").unwrap().as_str(),
            "https://vault.example.test"
        );
        assert_eq!(
            ServerAddress::parse("https://vault.example.test/").unwrap().as_str(),
            "https://vault.example.test"
        );
        assert_eq!(
            ServerAddress::parse("https://vault.example.test:8443")
                .unwrap()
                .as_str(),
            "https://vault.example.test:8443"
        );
    }

    #[test]
    fn refuses_everything_else() {
        let bad = [
            "http://vault.example.test",
            "https://user:pass@vault.example.test",
            "https://vault.example.test/app",
            "https://vault.example.test/?a=1",
            "https://vault.example.test/#frag",
            "https://vault.example.test?a=1",
            "vault.example.test",
            "",
            "ftp://vault.example.test",
        ];
        for text in bad {
            assert!(ServerAddress::parse(text).is_err(), "{text} must be refused");
        }
    }

    #[test]
    fn builds_urls() {
        let address = ServerAddress::parse("https://vault.example.test").unwrap();
        assert_eq!(address.url("/api/pair"), "https://vault.example.test/api/pair");
    }
}
