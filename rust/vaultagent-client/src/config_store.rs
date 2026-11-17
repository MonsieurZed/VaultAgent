//! Choix du stockage de la configuration : fichier en clair (`--config`, ou défaut Unix) ou fichier protégé
//! `remote-client.json` du dossier de données (Windows sans `--config`).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::approver_key::{ApproverKey, approval_public_key};
use crate::config::ClientConfig;
use crate::config_path::resolve;
use crate::error::ClientError;
use crate::protected_config::{load_protected, save_protected};
use crate::protector::system_protector;

/// Nom du fichier de configuration protégée.
pub const PROTECTED_FILE: &str = "remote-client.json";

/// Nom du fichier de la clé d'approbateur.
pub const APPROVER_FILE: &str = "approval-key.dpapi";

/// Emplacement et format de la configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigStore {
    /// Fichier JSON en clair `{"server","token"}`.
    Plain(PathBuf),
    /// Dossier de données : `remote-client.json` protégé et `approval-key.dpapi`.
    Protected(PathBuf),
}

impl ConfigStore {
    /// Détermine le stockage.
    ///
    /// Paramètres : le chemin de `--config`, le dossier personnel (`HOME`) et le dossier de données de l'application.
    /// Retour : `Plain` si `--config` est donné ; `Protected` sous Windows avec un dossier de données ; sinon le
    /// défaut Unix de `config_path::resolve`, ou une erreur d'usage.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn locate(
        explicit: Option<&Path>,
        home: Option<&OsStr>,
        local_data: Option<&Path>,
    ) -> Result<Self, ClientError> {
        match (explicit, local_data) {
            (None, Some(dir)) if cfg!(windows) => Ok(Self::Protected(dir.to_path_buf())),
            _ => resolve(explicit, home).map(Self::Plain),
        }
    }

    /// Lit la configuration.
    ///
    /// Retour : la configuration, ou `Config` en cas d'échec de lecture ou de déchiffrement.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn load(&self) -> Result<ClientConfig, ClientError> {
        match self {
            Self::Plain(path) => ClientConfig::load(path),
            Self::Protected(dir) => load_protected(&dir.join(PROTECTED_FILE), system_protector()?.as_ref()),
        }
    }

    /// Écrit la configuration.
    ///
    /// Paramètres : la configuration.
    /// Retour : `Ok`, ou `Config` en cas d'échec d'écriture ou de protection.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn save(&self, config: &ClientConfig) -> Result<(), ClientError> {
        match self {
            Self::Plain(path) => config.save(path),
            Self::Protected(dir) => save_protected(&dir.join(PROTECTED_FILE), system_protector()?.as_ref(), config),
        }
    }

    /// Clé publique de l'approbateur, créée à la première demande.
    ///
    /// Retour : la SPKI en base64 ; `Usage` avec `--config`, car la clé vit dans le dossier de données.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn approver_public_key(&self) -> Result<String, ClientError> {
        match self {
            Self::Plain(_) => Err(ClientError::Usage(
                "--approver needs the protected store: do not pass --config.".to_owned(),
            )),
            Self::Protected(dir) => approval_public_key(&dir.join(APPROVER_FILE), system_protector()?.as_ref()),
        }
    }

    /// Clé d'approbateur de ce poste, créée si elle manque.
    ///
    /// Retour : la clé ; `Usage` avec `--config`, car la clé vit dans le dossier de données.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn approver_key(&self) -> Result<ApproverKey, ClientError> {
        match self {
            Self::Plain(_) => Err(ClientError::Usage(
                "Approvals need the protected store: do not pass --config.".to_owned(),
            )),
            Self::Protected(dir) => ApproverKey::load_or_create(&dir.join(APPROVER_FILE), system_protector()?.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_config_is_always_plain() {
        let store = ConfigStore::locate(Some(Path::new("c.json")), None, Some(Path::new("data"))).unwrap();
        assert_eq!(store, ConfigStore::Plain(PathBuf::from("c.json")));
        assert_eq!(store.approver_public_key().unwrap_err().code(), "usage");
    }

    #[cfg(windows)]
    #[test]
    fn windows_without_config_uses_the_data_folder() {
        let store = ConfigStore::locate(None, None, Some(Path::new("data"))).unwrap();
        assert_eq!(store, ConfigStore::Protected(PathBuf::from("data")));
        assert_eq!(ConfigStore::locate(None, None, None).unwrap_err().code(), "usage");
    }

    #[cfg(windows)]
    #[test]
    fn protected_store_saves_loads_and_creates_the_approver_key() {
        use crate::server_address::ServerAddress;
        use crate::test_dir::{TestDir, sample_token};
        use crate::token::Token;
        let dir = TestDir::new();
        let store = ConfigStore::Protected(dir.path().join("VaultAgent"));
        let config = ClientConfig {
            server: ServerAddress::parse("https://vault.example.test").unwrap(),
            token: Token::parse(&sample_token()).unwrap(),
        };
        store.save(&config).unwrap();
        assert_eq!(store.load().unwrap(), config);
        let key = store.approver_public_key().unwrap();
        assert_eq!(store.approver_public_key().unwrap(), key);
    }
}
