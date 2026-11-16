//! Réponse scriptée du faux moteur : une valeur typée par forme de retour, ou une erreur.

use vaultagent_core::{ErrorCode, Result};

use crate::engine::fail;
use crate::item::Item;
use crate::passkey::PasskeyEntry;
use crate::secret::Secret;
use crate::structure::{Folder, Organization};

/// Réponse empilée pour le prochain appel ; sa forme doit correspondre à la méthode appelée.
#[derive(Debug, Clone)]
pub enum Reply {
    /// Succès sans valeur.
    Unit,
    /// Identifiant créé.
    Text(String),
    /// Secret (code TOTP).
    Secret(Secret),
    /// Une fiche.
    Item(Box<Item>),
    /// Des fiches.
    Items(Vec<Item>),
    /// Des dossiers.
    Folders(Vec<Folder>),
    /// Des organisations.
    Organizations(Vec<Organization>),
    /// Des passkeys.
    Passkeys(Vec<PasskeyEntry>),
    /// Échec du moteur.
    Failure(ErrorCode),
}

impl Reply {
    /// Erreur rendue quand la réponse ne correspond pas à la méthode : l'échec scripté, sinon `engine-refused`.
    ///
    /// Paramètres : aucun.
    /// Retour : l'erreur.
    fn mismatch(self) -> ErrorCode {
        match self {
            Self::Failure(code) => code,
            _ => fail("engine-refused"),
        }
    }

    /// Réponse d'une méthode sans valeur.
    ///
    /// # Errors
    ///
    /// L'échec scripté, ou `engine-refused` si la forme ne convient pas.
    pub(crate) fn into_unit(self) -> Result<()> {
        match self {
            Self::Unit => Ok(()),
            other => Err(other.mismatch()),
        }
    }

    /// Réponse d'une méthode qui rend un identifiant.
    ///
    /// # Errors
    ///
    /// L'échec scripté, ou `engine-refused` si la forme ne convient pas.
    pub(crate) fn into_text(self) -> Result<String> {
        match self {
            Self::Text(text) => Ok(text),
            other => Err(other.mismatch()),
        }
    }

    /// Réponse de `totp`.
    ///
    /// # Errors
    ///
    /// L'échec scripté, ou `engine-refused` si la forme ne convient pas.
    pub(crate) fn into_secret(self) -> Result<Secret> {
        match self {
            Self::Secret(secret) => Ok(secret),
            other => Err(other.mismatch()),
        }
    }

    /// Réponse de `item`.
    ///
    /// # Errors
    ///
    /// L'échec scripté, ou `engine-refused` si la forme ne convient pas.
    pub(crate) fn into_item(self) -> Result<Item> {
        match self {
            Self::Item(item) => Ok(*item),
            other => Err(other.mismatch()),
        }
    }

    /// Réponse de `items` et `trashed_items`.
    ///
    /// # Errors
    ///
    /// L'échec scripté, ou `engine-refused` si la forme ne convient pas.
    pub(crate) fn into_items(self) -> Result<Vec<Item>> {
        match self {
            Self::Items(items) => Ok(items),
            other => Err(other.mismatch()),
        }
    }

    /// Réponse de `folders`.
    ///
    /// # Errors
    ///
    /// L'échec scripté, ou `engine-refused` si la forme ne convient pas.
    pub(crate) fn into_folders(self) -> Result<Vec<Folder>> {
        match self {
            Self::Folders(folders) => Ok(folders),
            other => Err(other.mismatch()),
        }
    }

    /// Réponse de `organizations`.
    ///
    /// # Errors
    ///
    /// L'échec scripté, ou `engine-refused` si la forme ne convient pas.
    pub(crate) fn into_organizations(self) -> Result<Vec<Organization>> {
        match self {
            Self::Organizations(organizations) => Ok(organizations),
            other => Err(other.mismatch()),
        }
    }

    /// Réponse de `passkeys`.
    ///
    /// # Errors
    ///
    /// L'échec scripté, ou `engine-refused` si la forme ne convient pas.
    pub(crate) fn into_passkeys(self) -> Result<Vec<PasskeyEntry>> {
        match self {
            Self::Passkeys(entries) => Ok(entries),
            other => Err(other.mismatch()),
        }
    }
}
