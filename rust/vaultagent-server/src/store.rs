//! Magasin fichier d'un coffre : dossier privé, clé machine, lecture et écriture atomique de JSON, chiffrement des
//! valeurs. Le format de `/data` est celui du serveur Node, repris sans migration.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use serde::de::DeserializeOwned;
use vaultagent_core::encoding::hex_encode;
use vaultagent_core::protect::KEY_LEN;
use vaultagent_core::{DeviceRecord, Entropy, Protected, SystemEntropy, protect, unprotect};
use zeroize::Zeroizing;

use crate::account::Account;
use crate::error::{Result, ServerError};
use crate::private_fs::{create_new, create_private_dir, ensure_private};
use crate::sync::locked;

const KEY_FILE: &str = "machine.key";
const ACCOUNT_FILE: &str = "account.json";
const DEVICES_FILE: &str = "devices.json";

/// Dossier de données d'un coffre avec sa clé machine, son compte et ses appareils en mémoire.
pub struct Store {
    directory: PathBuf,
    pub(crate) key: Zeroizing<Vec<u8>>,
    account: Mutex<Option<Account>>,
    pub(crate) devices: Mutex<Vec<DeviceRecord>>,
    pub(crate) device_writes: Mutex<()>,
}

impl Store {
    /// Ouvre ou crée le magasin : dossier 0700, `machine.key` créée en exclusif, contrôle du mode et de la taille,
    /// puis lecture de `account.json` et `devices.json`.
    ///
    /// Paramètres : le dossier du coffre.
    /// Retour : le magasin.
    ///
    /// # Errors
    ///
    /// `private-key-permissions`, `private-key-invalid`, erreur d'entrée-sortie, ou JSON illisible.
    pub fn open(directory: &Path) -> Result<Self> {
        create_private_dir(directory)?;
        let key_path = directory.join(KEY_FILE);
        create_key(&key_path)?;
        ensure_private(&key_path)?;
        let key = Zeroizing::new(fs::read(&key_path)?);
        if key.len() != KEY_LEN {
            return Err(ServerError::PrivateKeyInvalid);
        }
        let store = Self {
            directory: directory.to_path_buf(),
            key,
            account: Mutex::new(None),
            devices: Mutex::new(Vec::new()),
            device_writes: Mutex::new(()),
        };
        *locked(&store.account) = store.load(ACCOUNT_FILE, None)?;
        *locked(&store.devices) = store.load(DEVICES_FILE, Vec::new())?;
        Ok(store)
    }

    /// Dossier du magasin.
    ///
    /// Paramètres : aucun.
    /// Retour : le chemin.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Lit un fichier JSON du dossier ; `None` s'il n'existe pas.
    ///
    /// Paramètres : le nom du fichier.
    /// Retour : la valeur lue, ou `None` si le fichier est absent.
    ///
    /// # Errors
    ///
    /// Toute erreur autre que l'absence du fichier, ou un JSON invalide.
    pub fn load_optional<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>> {
        match fs::read(self.directory.join(name)) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Lit un fichier JSON du dossier, ou rend la valeur de repli s'il est absent.
    ///
    /// Paramètres : le nom du fichier, la valeur de repli.
    /// Retour : la valeur lue ou le repli.
    ///
    /// # Errors
    ///
    /// Toute erreur autre que l'absence du fichier, ou un JSON invalide.
    pub fn load<T: DeserializeOwned>(&self, name: &str, fallback: T) -> Result<T> {
        Ok(self.load_optional(name)?.unwrap_or(fallback))
    }

    /// Écrit un fichier JSON de façon atomique : fichier temporaire `<nom>.<16 hex>.tmp` exclusif en 0600, puis
    /// `rename`. Le temporaire est supprimé si l'écriture échoue.
    ///
    /// Paramètres : le nom du fichier, la valeur à sérialiser (JSON compact).
    /// Retour : rien.
    ///
    /// # Errors
    ///
    /// Erreur de sérialisation, d'aléa ou d'entrée-sortie.
    pub fn save<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        let mut suffix = [0u8; 8];
        SystemEntropy.fill(&mut suffix)?;
        let temporary = self.directory.join(format!("{name}.{}.tmp", hex_encode(&suffix)));
        let outcome = write_then_rename(&temporary, &self.directory.join(name), &bytes);
        if outcome.is_err() {
            fs::remove_file(&temporary).ok();
        }
        outcome
    }

    /// Chiffre une valeur avec la clé machine.
    ///
    /// Paramètres : la valeur en clair, la source d'aléa du nonce.
    /// Retour : la valeur chiffrée au format du magasin.
    ///
    /// # Errors
    ///
    /// `engine-entropy` si l'aléa est indisponible.
    pub fn protect(&self, value: &str, entropy: &mut impl Entropy) -> Result<Protected> {
        Ok(protect(&self.key, value.as_bytes(), entropy)?)
    }

    /// Déchiffre une valeur du magasin.
    ///
    /// Paramètres : la valeur chiffrée.
    /// Retour : le texte en clair, effacé à la destruction.
    ///
    /// # Errors
    ///
    /// `invalid-key` si la valeur est altérée ou chiffrée par une autre clé, ou n'est pas de l'UTF-8.
    pub fn unprotect(&self, value: &Protected) -> Result<Zeroizing<String>> {
        let clear = unprotect(&self.key, value)?;
        let text = std::str::from_utf8(&clear).map_err(|_| vaultagent_core::ErrorCode::InvalidKey)?;
        Ok(Zeroizing::new(text.to_owned()))
    }

    /// Compte configuré.
    ///
    /// Paramètres : aucun.
    /// Retour : une copie du compte, absente si le coffre n'est pas configuré.
    #[must_use]
    pub fn account(&self) -> Option<Account> {
        locked(&self.account).clone()
    }

    /// Remplace le compte en mémoire (l'écriture de `account.json` se fait par [`Store::save`]).
    ///
    /// Paramètres : le nouveau compte.
    pub fn set_account(&self, account: Option<Account>) {
        *locked(&self.account) = account;
    }

    /// Appareils enregistrés.
    ///
    /// Paramètres : aucun.
    /// Retour : une copie de la liste.
    #[must_use]
    pub fn devices(&self) -> Vec<DeviceRecord> {
        locked(&self.devices).clone()
    }
}

/// Crée `machine.key` avec 32 octets aléatoires ; un fichier déjà présent est laissé tel quel.
///
/// Paramètres : le chemin de la clé.
/// Retour : rien.
///
/// # Errors
///
/// Erreur d'aléa ou d'entrée-sortie autre que `AlreadyExists`.
fn create_key(path: &Path) -> Result<()> {
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    SystemEntropy.fill(key.as_mut_slice())?;
    match create_new(path) {
        Ok(mut file) => Ok(file.write_all(key.as_slice())?),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Écrit le temporaire puis le renomme sur la cible.
///
/// Paramètres : le chemin temporaire, la cible, le contenu.
/// Retour : rien.
///
/// # Errors
///
/// Erreur d'entrée-sortie.
fn write_then_rename(temporary: &Path, target: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = create_new(temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    Ok(fs::rename(temporary, target)?)
}
