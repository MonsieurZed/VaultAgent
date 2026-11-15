//! Sélection du coffre d'un jeton : coffres où l'appareil est autorisé, choix par `vaultId`, métadonnées de
//! `/api/agent/vaults`. Le statut de chaque coffre (session) vient de l'appelant.

use crate::device_list::authenticate;
use crate::device_record::DeviceRecord;
use crate::error::{ErrorCode, Result};
use serde::Serialize;

/// Appareils d'un coffre tels que la sélection les voit.
#[derive(Debug, Clone, Copy)]
pub struct VaultDevices<'a> {
    /// Identifiant du coffre.
    pub id: &'a str,
    /// Profil du compte configuré ; `None` si le coffre n'a pas de compte (aucun appareil n'y est reconnu).
    pub profile: Option<&'a str>,
    /// Contenu du fichier des appareils du coffre.
    pub records: &'a [DeviceRecord],
}

impl VaultDevices<'_> {
    /// Cherche un appareil du profil courant par identifiant.
    ///
    /// Paramètres : l'identifiant de l'appareil.
    /// Retour : l'enregistrement, ou `None`.
    #[must_use]
    pub fn record(&self, device_id: &str) -> Option<&DeviceRecord> {
        let profile = self.profile?;
        self.records
            .iter()
            .find(|device| device.id == device_id && device.profile == profile)
    }
}

/// Identifiants des coffres dont le jeton authentifie un appareil.
///
/// Paramètres : l'en-tête `Authorization`, les coffres.
/// Retour : les identifiants autorisés, dans l'ordre des coffres.
#[must_use]
pub fn authorized(header: Option<&str>, vaults: &[VaultDevices<'_>]) -> Vec<String> {
    vaults
        .iter()
        .filter(|vault| {
            vault
                .profile
                .is_some_and(|profile| authenticate(header, vault.records, profile).is_ok())
        })
        .map(|vault| vault.id.to_owned())
        .collect()
}

/// Choisit le coffre d'une requête agent.
///
/// Paramètres : l'en-tête, le `vaultId` demandé s'il est présent, les coffres.
/// Retour : l'identifiant du coffre.
/// # Errors
///
/// `unauthorized` (aucun coffre autorisé, ou coffre demandé non autorisé) ; `vault-required` (plusieurs coffres
/// autorisés et aucun demandé).
pub fn select(header: Option<&str>, requested: Option<&str>, vaults: &[VaultDevices<'_>]) -> Result<String> {
    let allowed = authorized(header, vaults);
    let first = allowed.first().ok_or(ErrorCode::Unauthorized)?;
    match requested {
        None if allowed.len() == 1 => Ok(first.clone()),
        None => Err(ErrorCode::VaultRequired),
        Some(id) => allowed
            .iter()
            .find(|candidate| candidate.as_str() == id)
            .cloned()
            .ok_or(ErrorCode::Unauthorized),
    }
}

/// Entrée de la réponse de `/api/agent/vaults` : identifiant, nom, puis le statut de session à plat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VaultMetadata<S: Serialize> {
    /// Identifiant du coffre.
    pub id: String,
    /// Nom affiché.
    pub name: String,
    /// Statut de la session (`version`, `configured`, `unlocked`, `timeoutMinutes`, `expiresUtc`).
    #[serde(flatten)]
    pub status: S,
}

/// Métadonnées des coffres autorisés pour un jeton.
///
/// Paramètres : les coffres autorisés avec leur nom et leur statut, rendus par l'appelant.
/// Retour : la liste de la réponse.
#[must_use]
pub fn metadata<S: Serialize>(vaults: Vec<(String, String, S)>) -> Vec<VaultMetadata<S>> {
    vaults
        .into_iter()
        .map(|(id, name, status)| VaultMetadata { id, name, status })
        .collect()
}
