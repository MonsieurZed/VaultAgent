//! Configuration protégée `remote-client.json` : `{"server","protectedToken"}`, le jeton UTF-8 protégé puis en base64,
//! même fichier que le client .NET.

use std::path::Path;

use serde::{Deserialize, Serialize};
use vaultagent_core::encoding::{base64_decode, base64_encode};
use zeroize::Zeroizing;

use crate::config::ClientConfig;
use crate::error::ClientError;
use crate::private_file::write_private_atomic;
use crate::protector::Protector;
use crate::server_address::ServerAddress;
use crate::token::Token;

/// Forme du fichier, clés en camelCase comme `JsonSerializerDefaults.Web`.
#[derive(Serialize, Deserialize)]
struct Stored {
    server: String,
    #[serde(rename = "protectedToken")]
    protected_token: String,
}

/// Erreur de configuration au texte fixe.
fn invalid(text: &str) -> ClientError {
    ClientError::Config(text.to_owned())
}

/// Lit et déchiffre la configuration protégée.
///
/// Paramètres : le chemin du fichier et le protecteur.
/// Retour : la configuration, ou `Config` si le fichier est absent, mal formé, non déchiffrable ou porte une adresse
/// ou un jeton invalides.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn load_protected(path: &Path, protector: &dyn Protector) -> Result<ClientConfig, ClientError> {
    let bytes = std::fs::read(path)
        .map_err(|error| ClientError::Config(format!("Cannot read the protected configuration ({}).", error.kind())))?;
    let stored: Stored = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("Protected configuration is not valid JSON with server and protectedToken."))?;
    let encrypted = base64_decode(&stored.protected_token).ok_or_else(|| invalid("Protected token is not base64."))?;
    let clear = protector.unprotect(&encrypted)?;
    let text = std::str::from_utf8(&clear).map_err(|_| invalid("Protected token is not text."))?;
    let server = ServerAddress::parse(&stored.server)
        .map_err(|_| invalid("Protected configuration has an invalid server address."))?;
    Ok(ClientConfig {
        server,
        token: Token::parse(text)?,
    })
}

/// Protège et écrit la configuration de façon atomique.
///
/// Paramètres : le chemin du fichier, le protecteur et la configuration.
/// Retour : `Ok`, ou `Config` si la protection ou l'écriture échoue.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn save_protected(path: &Path, protector: &dyn Protector, config: &ClientConfig) -> Result<(), ClientError> {
    let encrypted = protector.protect(config.token.expose().as_bytes())?;
    let stored = Stored {
        server: config.server.as_str().to_owned(),
        protected_token: base64_encode(&encrypted),
    };
    let bytes =
        Zeroizing::new(serde_json::to_vec(&stored).map_err(|_| invalid("Cannot encode the protected configuration."))?);
    write_private_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protector::ReversingProtector;
    use crate::test_dir::{TestDir, sample_token};

    /// Aide de test.
    fn sample() -> ClientConfig {
        ClientConfig {
            server: ServerAddress::parse("https://vault.example.test").unwrap(),
            token: Token::parse(&sample_token()).unwrap(),
        }
    }

    #[test]
    fn save_then_load_round_trips_and_hides_the_token() {
        let dir = TestDir::new();
        let path = dir.path().join("remote-client.json");
        save_protected(&path, &ReversingProtector, &sample()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("{\"server\":\"https://vault.example.test\",\"protectedToken\":\""));
        assert!(!text.contains(&sample_token()));
        assert_eq!(load_protected(&path, &ReversingProtector).unwrap(), sample());
    }

    #[test]
    fn bad_content_is_refused() {
        let dir = TestDir::new();
        let path = dir.path().join("remote-client.json");
        write_private_atomic(&path, b"{\"server\":\"https://vault.example.test\"}").unwrap();
        assert_eq!(load_protected(&path, &ReversingProtector).unwrap_err().code(), "config");
        write_private_atomic(
            &path,
            b"{\"server\":\"https://vault.example.test\",\"protectedToken\":\"%%%\"}",
        )
        .unwrap();
        assert_eq!(load_protected(&path, &ReversingProtector).unwrap_err().code(), "config");
        assert!(load_protected(&dir.path().join("absent.json"), &ReversingProtector).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_file_written_like_the_dotnet_client_is_read() {
        use crate::protector::DpapiProtector;
        let dir = TestDir::new();
        let path = dir.path().join("remote-client.json");
        let encrypted = vaultagent_win32::protect(
            sample_token().as_bytes(),
            b"VaultAgent.Remote.v1",
            vaultagent_win32::Scope::CurrentUser,
        )
        .unwrap();
        let text = format!(
            "{{\"server\":\"https://vault.example.test\",\"protectedToken\":\"{}\"}}",
            base64_encode(&encrypted)
        );
        write_private_atomic(&path, text.as_bytes()).unwrap();
        assert_eq!(load_protected(&path, &DpapiProtector).unwrap(), sample());
        save_protected(&path, &DpapiProtector, &sample()).unwrap();
        assert_eq!(load_protected(&path, &DpapiProtector).unwrap(), sample());
    }
}
