//! Opérations sur la liste des appareils (`devices.mjs`) : authentification, liste publique, révocation, acceptation
//! automatique, enrôlements Hello et élévation. Chaque mutation rend la nouvelle liste sans rien écrire.

use crate::device_record::{DeviceRecord, HelloCredential, PublicDevice};
use crate::device_token::parse_bearer;
use crate::encoding::{digest, same};
use crate::error::{ErrorCode, Result};

/// Authentifie un en-tête `Authorization: Bearer va1.…`.
///
/// Paramètres : l'en-tête s'il est présent, la liste des appareils, le profil courant.
/// Retour : l'identifiant de l'appareil.
/// # Errors
///
/// `unauthorized` si la forme est fausse, l'appareil inconnu, d'un autre profil ou l'empreinte différente.
pub fn authenticate(header: Option<&str>, records: &[DeviceRecord], profile: &str) -> Result<String> {
    let (token, id) = parse_bearer(header).ok_or(ErrorCode::Unauthorized)?;
    let device = records
        .iter()
        .find(|value| value.id == id)
        .ok_or(ErrorCode::Unauthorized)?;
    if device.profile == profile && same(&device.hash, &digest(token.as_bytes())) {
        Ok(device.id.clone())
    } else {
        Err(ErrorCode::Unauthorized)
    }
}

/// Liste publique des appareils du profil courant.
///
/// Paramètres : la liste, le profil.
/// Retour : identifiant, nom, date et acceptation automatique de chaque appareil.
#[must_use]
pub fn list(records: &[DeviceRecord], profile: &str) -> Vec<PublicDevice> {
    records
        .iter()
        .filter(|value| value.profile == profile)
        .map(DeviceRecord::public)
        .collect()
}

/// Retire un appareil.
///
/// Paramètres : la liste, l'identifiant à révoquer (sans effet s'il est inconnu).
/// Retour : la nouvelle liste.
#[must_use]
pub fn revoke(records: &[DeviceRecord], id: &str) -> Vec<DeviceRecord> {
    records.iter().filter(|device| device.id != id).cloned().collect()
}

/// Trouve la position d'un appareil du profil courant.
fn position(records: &[DeviceRecord], id: &str, profile: &str) -> Option<usize> {
    records
        .iter()
        .position(|value| value.id == id && value.profile == profile)
}

/// Indique si un champ texte facultatif est renseigné (vrai au sens JavaScript).
fn present(field: Option<&str>) -> bool {
    field.is_some_and(|text| !text.is_empty())
}

/// Active ou coupe l'acceptation automatique d'un appareil.
///
/// Paramètres : la liste, l'identifiant, le profil, le nouvel état.
/// Retour : la nouvelle liste.
/// # Errors
///
/// `invalid-device` si l'appareil n'existe pas dans le profil.
pub fn set_automatic(records: &[DeviceRecord], id: &str, profile: &str, enabled: bool) -> Result<Vec<DeviceRecord>> {
    let index = position(records, id, profile).ok_or(ErrorCode::InvalidDevice)?;
    let mut next = records.to_vec();
    next[index].auto_unlock = Some(enabled);
    Ok(next)
}

/// Coupe l'acceptation automatique de tous les appareils.
///
/// Paramètres : la liste.
/// Retour : la nouvelle liste.
#[must_use]
pub fn clear_automatic(records: &[DeviceRecord]) -> Vec<DeviceRecord> {
    records
        .iter()
        .cloned()
        .map(|mut device| {
            device.auto_unlock = Some(false);
            device
        })
        .collect()
}

/// Autorise l'enrôlement de Windows Hello pour un appareil qui a une clé d'approbateur.
///
/// Paramètres : la liste, l'identifiant, le profil.
/// Retour : la nouvelle liste.
/// # Errors
///
/// `hello-enrollment-refused` si l'appareil est absent ou sans clé d'approbateur.
pub fn authorize_hello_enrollment(records: &[DeviceRecord], id: &str, profile: &str) -> Result<Vec<DeviceRecord>> {
    let index = position(records, id, profile)
        .filter(|index| present(records[*index].approver_public_key.as_deref()))
        .ok_or(ErrorCode::HelloEnrollmentRefused)?;
    let mut next = records.to_vec();
    next[index].hello_enrollment_allowed = Some(true);
    Ok(next)
}

/// Enregistre le justificatif Windows Hello d'un appareil et referme l'autorisation.
///
/// Paramètres : la liste, l'identifiant, le profil, le justificatif.
/// Retour : la nouvelle liste.
/// # Errors
///
/// `hello-enrollment-refused` si l'appareil est absent, sans clé d'approbateur ou déjà enrôlé.
pub fn enroll_hello(
    records: &[DeviceRecord],
    id: &str,
    profile: &str,
    credential: HelloCredential,
) -> Result<Vec<DeviceRecord>> {
    let index = position(records, id, profile)
        .filter(|index| {
            present(records[*index].approver_public_key.as_deref()) && records[*index].hello_credential.is_none()
        })
        .ok_or(ErrorCode::HelloEnrollmentRefused)?;
    let mut next = records.to_vec();
    next[index].hello_enrollment_allowed = Some(false);
    next[index].hello_credential = Some(credential);
    Ok(next)
}

/// Enregistre la clé publique d'élévation d'un appareil.
///
/// Paramètres : la liste, l'identifiant, le profil, la clé SPKI en base64 déjà validée.
/// Retour : la nouvelle liste.
/// # Errors
///
/// `elevation-enrollment-refused` si l'appareil est absent, sans clé d'approbateur ou déjà enrôlé.
pub fn enroll_elevation(
    records: &[DeviceRecord],
    id: &str,
    profile: &str,
    public_key: &str,
) -> Result<Vec<DeviceRecord>> {
    let index = position(records, id, profile)
        .filter(|index| {
            present(records[*index].approver_public_key.as_deref())
                && !present(records[*index].elevation_public_key.as_deref())
        })
        .ok_or(ErrorCode::ElevationEnrollmentRefused)?;
    let mut next = records.to_vec();
    next[index].elevation_public_key = Some(public_key.to_owned());
    Ok(next)
}
