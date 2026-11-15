//! Registre des coffres (`vaults.json`) : liste `[{id, name}]`, validation à l'ouverture, ajout et renommage.
//! Pur : l'appelant charge le fichier, enregistre le registre rendu et crée le dossier d'un coffre neuf.

use crate::device_token::random_uuid;
use crate::entropy::Entropy;
use crate::error::{ErrorCode, Result};
use crate::jsval::trim;
use crate::name::BoundedName;
use crate::uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Nombre maximal de coffres.
pub const MAX_VAULTS: usize = 8;

/// Longueur maximale du nom d'un coffre, en unités UTF-16.
pub const NAME_MAX: usize = 80;

/// Identifiant du coffre historique, stocké à la racine de `/data`.
pub const DEFAULT_ID: &str = "default";

/// Nom du coffre historique quand `vaults.json` n'existe pas.
pub const DEFAULT_NAME: &str = "Coffre existant";

/// Un coffre du registre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultEntry {
    /// `default` ou UUID.
    pub id: String,
    /// Nom affiché.
    pub name: String,
}

/// Résultat de [`VaultRegistry::save_name`] : le registre à enregistrer et le coffre concerné.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedVault {
    /// Nouveau registre complet.
    pub registry: VaultRegistry,
    /// Coffre ajouté ou renommé.
    pub entry: VaultEntry,
    /// Vrai si le coffre vient d'être ajouté (le dossier `vaults/<id>` est à créer).
    pub created: bool,
}

/// Registre validé : 1 à 8 coffres, identifiants uniques, `default` présent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultRegistry {
    entries: Vec<VaultEntry>,
}

impl Default for VaultRegistry {
    /// Registre d'un compte existant : le seul coffre `default`.
    fn default() -> Self {
        Self {
            entries: vec![VaultEntry {
                id: DEFAULT_ID.to_owned(),
                name: DEFAULT_NAME.to_owned(),
            }],
        }
    }
}

impl VaultRegistry {
    /// Valide le contenu de `vaults.json`.
    ///
    /// Paramètres : le JSON lu, ou `None` si le fichier n'existe pas.
    /// Retour : le registre (celui par défaut si le fichier est absent).
    /// # Errors
    ///
    /// `invalid-vault-registry` (pas une liste d'objets `{id, name}`, vide, plus de 8, doublon, `default`
    /// absent) ; `invalid-vault` (identifiant autre que `default` qui n'est pas un UUID).
    pub fn init(raw: Option<&Value>) -> Result<Self> {
        let Some(raw) = raw else {
            return Ok(Self::default());
        };
        let entries: Vec<VaultEntry> =
            serde_json::from_value(raw.clone()).map_err(|_| ErrorCode::InvalidVaultRegistry)?;
        let unique = entries
            .iter()
            .enumerate()
            .all(|(index, entry)| entries[..index].iter().all(|other| other.id != entry.id));
        let shaped = !entries.is_empty()
            && entries.len() <= MAX_VAULTS
            && unique
            && entries.iter().any(|entry| entry.id == DEFAULT_ID);
        if !shaped {
            return Err(ErrorCode::InvalidVaultRegistry);
        }
        if entries
            .iter()
            .any(|entry| entry.id != DEFAULT_ID && Uuid::parse(&entry.id).is_none())
        {
            return Err(ErrorCode::InvalidVault);
        }
        Ok(Self { entries })
    }

    /// Coffres du registre, dans l'ordre du fichier.
    #[must_use]
    pub fn entries(&self) -> &[VaultEntry] {
        &self.entries
    }

    /// Cherche un coffre par identifiant.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&VaultEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Forme JSON du fichier `vaults.json`.
    ///
    /// Retour : la liste `[{id, name}]`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        serde_json::to_value(&self.entries).unwrap_or(Value::Null)
    }

    /// Ajoute un coffre (sans `id`) ou renomme un coffre existant.
    ///
    /// Paramètres : l'identifiant à renommer (`None` pour ajouter), le nom (`None` s'il n'est pas un texte),
    /// la source d'aléa pour l'identifiant d'un coffre neuf.
    /// Retour : le nouveau registre, le coffre touché et s'il est neuf ; le nom est enregistré sans espaces de
    /// bord. Le registre d'origine n'est pas modifié : l'appelant l'enregistre puis le remplace.
    /// # Errors
    ///
    /// `invalid-vault-name` (absent, vide, plus de 80 unités UTF-16, caractère de contrôle) ; `vault-limit`
    /// (ajout alors que 8 coffres existent) ; `vault-missing` (identifiant inconnu) ; `engine-entropy`.
    pub fn save_name(&self, id: Option<&str>, name: Option<&str>, entropy: &mut impl Entropy) -> Result<SavedVault> {
        let name = name
            .and_then(|text| BoundedName::parse(text, NAME_MAX))
            .ok_or(ErrorCode::InvalidVaultName)?;
        let trimmed = trim(name.as_str()).to_owned();
        let mut registry = self.clone();
        let (entry, created) = match id {
            None if self.entries.len() >= MAX_VAULTS => return Err(ErrorCode::VaultLimit),
            None => {
                let entry = VaultEntry {
                    id: random_uuid(entropy)?,
                    name: trimmed,
                };
                registry.entries.push(entry.clone());
                (entry, true)
            }
            Some(known) => {
                let slot = registry
                    .entries
                    .iter_mut()
                    .find(|entry| entry.id == known)
                    .ok_or(ErrorCode::VaultMissing)?;
                slot.name = trimmed;
                (slot.clone(), false)
            }
        };
        Ok(SavedVault {
            registry,
            entry,
            created,
        })
    }
}
