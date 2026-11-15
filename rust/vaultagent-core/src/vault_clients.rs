//! Clients partagés entre coffres : refus d'un compte déjà utilisé, liste des appareils connus, droit accordé
//! à un appareil sur un autre coffre. Les mutations rendent la nouvelle liste, l'écriture est à l'appelant.

use crate::device_list::list;
use crate::device_record::DeviceRecord;
use crate::error::{ErrorCode, Result};
use crate::https::https_origin;
use crate::vault_select::VaultDevices;
use serde::Serialize;

/// Nombre maximal d'appareils d'un coffre.
pub const MAX_DEVICES: usize = 32;

/// Compte configuré d'un coffre : serveur Vaultwarden et courriel.
#[derive(Debug, Clone, Copy)]
pub struct AccountRef<'a> {
    /// URL du serveur.
    pub server: &'a str,
    /// Courriel du compte.
    pub email: &'a str,
}

/// Origine d'une URL de serveur ; le texte brut sert de repli quand elle n'est pas lisible.
fn origin_of(server: &str) -> String {
    https_origin(server).unwrap_or_else(|| server.to_owned())
}

/// Refuse un compte déjà configuré dans un autre coffre : même origine de serveur et même courriel, sans
/// tenir compte de la casse du courriel.
///
/// Paramètres : les comptes des autres coffres, le compte à configurer.
/// Retour : rien.
/// # Errors
///
/// `duplicate-account` si un autre coffre porte déjà ce compte.
pub fn ensure_unique_account(others: &[AccountRef<'_>], candidate: &AccountRef<'_>) -> Result<()> {
    let origin = origin_of(candidate.server);
    let email = candidate.email.to_lowercase();
    let taken = others
        .iter()
        .any(|other| origin_of(other.server) == origin && other.email.to_lowercase() == email);
    if taken {
        Err(ErrorCode::DuplicateAccount)
    } else {
        Ok(())
    }
}

/// Appareil proposé à l'administrateur pour un autre coffre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Client {
    /// Identifiant de l'appareil.
    pub id: String,
    /// Nom affiché.
    pub name: String,
}

/// Appareils connus de tous les coffres, une seule fois chacun.
///
/// Paramètres : les coffres.
/// Retour : identifiant et nom, au premier coffre qui connaît l'appareil.
#[must_use]
pub fn available_clients(vaults: &[VaultDevices<'_>]) -> Vec<Client> {
    let mut clients: Vec<Client> = Vec::new();
    for vault in vaults {
        let Some(profile) = vault.profile else {
            continue;
        };
        for device in list(vault.records, profile) {
            if clients.iter().all(|known| known.id != device.id) {
                clients.push(Client {
                    id: device.id,
                    name: device.name,
                });
            }
        }
    }
    clients
}

/// Résultat d'un droit accordé : la liste d'appareils du coffre cible, et si elle a changé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// Liste à enregistrer pour le coffre cible.
    pub records: Vec<DeviceRecord>,
    /// Faux quand l'appareil avait déjà accès (rien à écrire).
    pub changed: bool,
}

/// Accorde à un appareil l'accès à un autre coffre : copie de son enregistrement, rattachée au profil de la
/// cible, acceptation automatique coupée.
///
/// Paramètres : tous les coffres, l'identifiant du coffre cible, l'identifiant de l'appareil.
/// Retour : la liste du coffre cible. La réponse du protocole est `{granted: true}` dans tous les cas de succès.
/// # Errors
///
/// `vault-missing` (cible inconnue) ; `not-configured` (cible sans compte) ; `invalid-device` (appareil inconnu de
/// tous les coffres) ; `device-limit` (32 appareils déjà).
pub fn grant(vaults: &[VaultDevices<'_>], target_id: &str, device_id: &str) -> Result<Grant> {
    let target = vaults
        .iter()
        .find(|vault| vault.id == target_id)
        .ok_or(ErrorCode::VaultMissing)?;
    let profile = target.profile.ok_or(ErrorCode::NotConfigured)?;
    let source = vaults
        .iter()
        .find_map(|vault| vault.record(device_id))
        .ok_or(ErrorCode::InvalidDevice)?;
    if target.record(device_id).is_some() {
        return Ok(Grant {
            records: target.records.to_vec(),
            changed: false,
        });
    }
    if target.records.len() >= MAX_DEVICES {
        return Err(ErrorCode::DeviceLimit);
    }
    let mut records = target.records.to_vec();
    records.push(DeviceRecord {
        profile: profile.to_owned(),
        auto_unlock: Some(false),
        ..source.clone()
    });
    Ok(Grant { records, changed: true })
}
