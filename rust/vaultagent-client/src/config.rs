//! Configuration du client : `{"server","token"}`, même fichier et même format que le client Node.

use std::path::Path;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::error::ClientError;
use crate::private_file::{check_private, write_private_atomic};
use crate::server_address::ServerAddress;
use crate::token::Token;

/// Adresse du serveur et jeton de l'appareil associé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    /// Origine du serveur.
    pub server: ServerAddress,
    /// Jeton de l'appareil.
    pub token: Token,
}

/// Forme du fichier à la lecture ; le jeton est effacé à la destruction.
#[derive(Deserialize)]
struct Stored {
    server: String,
    token: String,
}

impl Drop for Stored {
    /// Efface le jeton lu.
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

/// Forme du fichier à l'écriture, dans l'ordre de Node.
#[derive(Serialize)]
struct Written<'a> {
    server: &'a str,
    token: &'a str,
}

impl ClientConfig {
    /// Lit et valide la configuration.
    ///
    /// Paramètres : le chemin du fichier.
    /// Retour : la configuration, ou `Config` si le fichier est absent, non privé (Unix), illisible, ou porte une
    /// adresse ou un jeton invalides.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn load(path: &Path) -> Result<Self, ClientError> {
        check_private(path)?;
        let bytes = Zeroizing::new(std::fs::read(path).map_err(|error| {
            ClientError::Config(format!("Cannot read the client configuration ({}).", error.kind()))
        })?);
        let stored: Stored = serde_json::from_slice(&bytes).map_err(|_| {
            ClientError::Config("Client configuration is not valid JSON with server and token.".to_owned())
        })?;
        let server = ServerAddress::parse(&stored.server)
            .map_err(|_| ClientError::Config("Client configuration has an invalid server address.".to_owned()))?;
        Ok(Self {
            server,
            token: Token::parse(&stored.token)?,
        })
    }

    /// Écrit la configuration de façon atomique, en `0600`.
    ///
    /// Paramètres : le chemin du fichier.
    /// Retour : `Ok`, ou `Config` si l'écriture échoue.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn save(&self, path: &Path) -> Result<(), ClientError> {
        let written = Written {
            server: self.server.as_str(),
            token: self.token.expose(),
        };
        let bytes = Zeroizing::new(
            serde_json::to_vec(&written)
                .map_err(|_| ClientError::Config("Cannot encode the client configuration.".to_owned()))?,
        );
        write_private_atomic(path, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir::TestDir;
    use crate::test_dir::sample_token;

    /// Aide de test.
    fn sample() -> ClientConfig {
        ClientConfig {
            server: ServerAddress::parse("https://vault.example.test").unwrap(),
            token: Token::parse(&sample_token()).unwrap(),
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = TestDir::new();
        let path = dir.path().join("vaultagent").join("client.json");
        sample().save(&path).unwrap();
        assert_eq!(ClientConfig::load(&path).unwrap(), sample());
    }

    #[test]
    fn file_format_matches_the_node_client() {
        let dir = TestDir::new();
        let path = dir.path().join("client.json");
        sample().save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            format!(
                "{{\"server\":\"https://vault.example.test\",\"token\":\"{}\"}}",
                sample_token()
            )
        );
    }

    #[test]
    fn node_written_file_is_read() {
        let dir = TestDir::new();
        let path = dir.path().join("client.json");
        let text = format!(
            "{{\"server\":\"https://vault.example.test\",\"token\":\"{}\"}}",
            sample_token()
        );
        write_private_atomic(&path, text.as_bytes()).unwrap();
        assert_eq!(ClientConfig::load(&path).unwrap(), sample());
    }

    #[test]
    fn invalid_content_is_refused_without_echoing_the_token() {
        let dir = TestDir::new();
        let path = dir.path().join("client.json");
        write_private_atomic(&path, b"{\"server\":\"http://x.test\",\"token\":\"secret-value\"}").unwrap();
        let error = ClientConfig::load(&path).unwrap_err();
        assert_eq!(error.code(), "config");
        assert!(!error.message().contains("secret-value"));
        write_private_atomic(&path, b"not json").unwrap();
        assert!(ClientConfig::load(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn non_private_file_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TestDir::new();
        let path = dir.path().join("client.json");
        sample().save(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(ClientConfig::load(&path).unwrap_err().code(), "config");
    }
}
