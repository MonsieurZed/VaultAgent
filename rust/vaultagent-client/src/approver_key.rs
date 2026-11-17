//! Clé d'approbateur RSA 2048 (PKCS#8 DER protégé, fichier `approval-key.dpapi`) : création à la première demande,
//! clé publique SPKI en base64 et signature RSA-PSS SHA-256 de sel 32, comme `RemoteApprovals.cs`.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use vaultagent_core::{
    RsaPrivateKey, SystemEntropy, export_pkcs8, generate_rsa, import_pkcs8, public_key_spki, sign_pss,
};
use zeroize::Zeroizing;

use crate::error::ClientError;
use crate::protector::Protector;

const KEY_BITS: usize = 2048;

/// Erreur de configuration au texte fixe, sans chemin ni contenu.
fn failure(text: &str) -> ClientError {
    ClientError::Config(text.to_owned())
}

/// Clé privée de l'approbateur, effacée à la destruction par la crate `rsa`.
pub struct ApproverKey(RsaPrivateKey);

/// Écrit la clé protégée dans un temporaire puis la lie à la cible : la création échoue si la cible existe déjà.
fn create_new(path: &Path, protected: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{}.tmp", std::process::id()));
    let temporary = PathBuf::from(name);
    let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
    file.write_all(protected)?;
    file.sync_all()?;
    drop(file);
    let linked = fs::hard_link(&temporary, path);
    fs::remove_file(&temporary).unwrap_or_default();
    linked
}

/// Charge une clé protégée depuis un fichier.
fn read(path: &Path, protector: &dyn Protector) -> Result<ApproverKey, ClientError> {
    let bytes = fs::read(path).map_err(|_| failure("Cannot read the approval key."))?;
    let clear: Zeroizing<Vec<u8>> = protector.unprotect(&bytes)?;
    import_pkcs8(&clear)
        .map(ApproverKey)
        .map_err(|_| failure("The approval key file is damaged."))
}

impl ApproverKey {
    /// Charge la clé d'approbateur, ou la crée si le fichier est absent.
    ///
    /// Paramètres : le chemin du fichier et le protecteur.
    /// Retour : la clé ; `Config` si le fichier est corrompu, non déchiffrable, ou si la création échoue. Un fichier
    /// créé en parallèle par un autre processus est relu.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn load_or_create(path: &Path, protector: &dyn Protector) -> Result<Self, ClientError> {
        if path.exists() {
            return read(path, protector);
        }
        let key =
            generate_rsa(KEY_BITS, &mut SystemEntropy).map_err(|_| failure("Cannot generate the approval key."))?;
        let der = export_pkcs8(&key).map_err(|_| failure("Cannot encode the approval key."))?;
        let protected = protector.protect(&der)?;
        match create_new(path, &protected) {
            Ok(()) => Ok(Self(key)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => read(path, protector),
            Err(_) => Err(failure("Cannot store the approval key.")),
        }
    }

    /// Clé publique de l'approbateur.
    ///
    /// Retour : la SPKI DER en base64, ou `Config` si l'encodage échoue.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn public_key(&self) -> Result<String, ClientError> {
        public_key_spki(&self.0).map_err(|_| failure("Cannot encode the approval public key."))
    }

    /// Signe un message en RSA-PSS SHA-256 de sel 32.
    ///
    /// Paramètres : le message, par exemple celui de `vaultagent_core::decision_message`.
    /// Retour : la signature en base64, ou `Config` si l'aléa est indisponible.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn sign(&self, message: &str) -> Result<String, ClientError> {
        sign_pss(&self.0, message, &mut SystemEntropy).map_err(|_| failure("Cannot sign with the approval key."))
    }
}

/// Clé publique d'approbateur du fichier, créé au besoin.
///
/// Paramètres : le chemin du fichier de clé et le protecteur.
/// Retour : la SPKI en base64, ou l'erreur de chargement.
///
/// # Errors
///
/// Les erreurs possibles sont celles de [`ApproverKey::load_or_create`] et [`ApproverKey::public_key`].
pub fn approval_public_key(path: &Path, protector: &dyn Protector) -> Result<String, ClientError> {
    ApproverKey::load_or_create(path, protector)?.public_key()
}

/// Signature d'un message par la clé d'approbateur du fichier, créée au besoin.
///
/// Paramètres : le chemin du fichier de clé, le protecteur et le message.
/// Retour : la signature en base64, ou l'erreur de chargement.
///
/// # Errors
///
/// Les erreurs possibles sont celles de [`ApproverKey::load_or_create`] et [`ApproverKey::sign`].
pub fn approval_sign(path: &Path, protector: &dyn Protector, message: &str) -> Result<String, ClientError> {
    ApproverKey::load_or_create(path, protector)?.sign(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protector::ReversingProtector;
    use crate::test_dir::TestDir;
    use vaultagent_core::{ErrorCode, decision_message, verify_pss};

    #[test]
    fn key_is_created_once_then_reloaded_with_a_stable_public_key() {
        let dir = TestDir::new();
        let path = dir.path().join("approval-key.dpapi");
        let first = approval_public_key(&path, &ReversingProtector).unwrap();
        assert!(path.exists());
        assert_eq!(approval_public_key(&path, &ReversingProtector).unwrap(), first);
        assert!(fs::read(&path).unwrap().len() > 1000);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn signature_is_accepted_by_the_core_verifier_and_only_for_its_message() {
        let dir = TestDir::new();
        let path = dir.path().join("approval-key.dpapi");
        let public = approval_public_key(&path, &ReversingProtector).unwrap();
        let message = decision_message("https://vault.example.test", "default", "req-1", "chal-1", true, false);
        let signature = approval_sign(&path, &ReversingProtector, &message).unwrap();
        verify_pss(&public, &message, &signature, ErrorCode::InvalidKey).unwrap();
        assert!(verify_pss(&public, "autre message", &signature, ErrorCode::InvalidKey).is_err());
    }

    #[test]
    fn corrupted_file_is_refused_and_left_untouched() {
        let dir = TestDir::new();
        let path = dir.path().join("approval-key.dpapi");
        fs::write(&path, b"not a protected key").unwrap();
        let error = approval_public_key(&path, &ReversingProtector).unwrap_err();
        assert_eq!(error.code(), "config");
        assert_eq!(fs::read(&path).unwrap(), b"not a protected key");
    }

    #[test]
    fn decision_message_matches_the_dotnet_relaxed_escaping() {
        let message = decision_message("https://x.test", "default", "r", "c<>&'+\u{e9}", true, false);
        assert_eq!(
            message,
            "[\"VaultAgent.Unlock.v2\",\"https://x.test\",\"default\",\"r\",\"c<>&'+\u{e9}\",true,false]"
        );
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_protected_key_file_round_trips() {
        use crate::protector::DpapiProtector;
        let dir = TestDir::new();
        let path = dir.path().join("approval-key.dpapi");
        let first = approval_public_key(&path, &DpapiProtector).unwrap();
        assert_eq!(approval_public_key(&path, &DpapiProtector).unwrap(), first);
        let mut damaged = fs::read(&path).unwrap();
        let middle = damaged.len() / 2;
        damaged[middle] ^= 0xff;
        fs::write(&path, damaged).unwrap();
        assert_eq!(
            approval_public_key(&path, &DpapiProtector).unwrap_err().code(),
            "config"
        );
    }
}
