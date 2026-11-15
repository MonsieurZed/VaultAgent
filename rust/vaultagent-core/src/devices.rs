//! Codes d'association émis par l'administrateur (18 octets `base64url`, 300 s, 8 en attente, 10 essais par minute)
//! et enrôlement d'un appareil (`devices.mjs`). L'état des codes vit en mémoire ; la liste des appareils est
//! passée en paramètre et rendue modifiée, l'écriture du fichier se fait ailleurs.

use crate::device_record::{DeviceRecord, iso_from_millis};
use crate::device_token::issue_token;
use crate::encoding::{base64url_encode, digest};
use crate::entropy::Entropy;
use crate::error::{ErrorCode, Result};
use crate::jsval::{is_blank, trim, utf16_len};
use serde::Serialize;
use zeroize::Zeroizing;

/// Durée de vie d'un code d'association, en millisecondes.
pub const CODE_LIFETIME_MS: u64 = 300_000;
/// Codes d'association en attente au plus.
pub const MAX_PENDING_CODES: usize = 8;
/// Essais d'association par minute au plus.
pub const MAX_ATTEMPTS_PER_MINUTE: usize = 10;
/// Appareils enregistrés au plus, tous profils confondus.
pub const MAX_DEVICES: usize = 32;
const WINDOW_MS: u64 = 60_000;
const MAX_NAME_UNITS: usize = 80;
const CODE_BYTES: usize = 18;

/// Code d'association remis à l'administrateur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedCode {
    /// Code à saisir sur le client, `base64url`.
    pub code: String,
    /// Durée de validité restante en secondes.
    pub expires_in_seconds: u64,
}

/// Résultat d'un enrôlement : le jeton n'est remis qu'une fois, seule son empreinte est conservée.
pub struct Enrolled {
    /// Jeton complet, effacé à la destruction.
    pub token: Zeroizing<String>,
    /// Identifiant de l'appareil créé.
    pub device_id: String,
}

struct PendingCode {
    hash: String,
    name: String,
    profile: String,
    expires: u64,
}

/// État en mémoire des codes d'association et des essais récents.
#[derive(Default)]
pub struct Devices {
    pairings: Vec<PendingCode>,
    attempts: Vec<u64>,
}

impl Devices {
    /// Crée un état vide.
    ///
    /// Paramètres : aucun.
    /// Retour : l'état.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Émet un code d'association à usage unique pour un nom d'appareil.
    ///
    /// Paramètres : le nom (80 unités UTF-16 au plus, non vide une fois rogné), le profil courant, l'heure en
    /// millisecondes, la source d'aléa.
    /// Retour : le code et sa durée.
    /// # Errors
    ///
    /// `invalid-device` pour un nom vide ou trop long ; `pairing-limit` si 8 codes sont déjà en attente ;
    /// `engine-entropy` si l'aléa est indisponible.
    pub fn issue(&mut self, name: &str, profile: &str, now: u64, entropy: &mut impl Entropy) -> Result<IssuedCode> {
        if is_blank(name) || utf16_len(name) > MAX_NAME_UNITS {
            return Err(ErrorCode::InvalidDevice);
        }
        self.pairings.retain(|entry| entry.expires >= now);
        if self.pairings.len() >= MAX_PENDING_CODES {
            return Err(ErrorCode::PairingLimit);
        }
        let mut bytes = [0u8; CODE_BYTES];
        entropy.fill(&mut bytes)?;
        let code = base64url_encode(&bytes);
        self.pairings.push(PendingCode {
            hash: digest(code.as_bytes()),
            name: trim(name).to_owned(),
            profile: profile.to_owned(),
            expires: now + CODE_LIFETIME_MS,
        });
        Ok(IssuedCode {
            code,
            expires_in_seconds: CODE_LIFETIME_MS / 1000,
        })
    }

    /// Consomme un code d'association et enrôle l'appareil.
    ///
    /// Paramètres : le code saisi (absent compte pour un texte vide), le profil courant, l'heure, la liste actuelle
    /// des appareils, la source d'aléa.
    /// Retour : la nouvelle liste et le jeton.
    /// # Errors
    ///
    /// `pairing-rate-limit` au onzième essai de la minute ; `pairing-refused` si le code est inconnu, expiré, déjà
    /// utilisé ou d'un autre profil (le code est consommé dans tous les cas) ; `device-limit` ; `engine-entropy`.
    pub fn pair(
        &mut self,
        code: Option<&str>,
        profile: &str,
        now: u64,
        records: &[DeviceRecord],
        entropy: &mut impl Entropy,
    ) -> Result<(Vec<DeviceRecord>, Enrolled)> {
        self.attempts.retain(|time| time + WINDOW_MS > now);
        if self.attempts.len() >= MAX_ATTEMPTS_PER_MINUTE {
            return Err(ErrorCode::PairingRateLimit);
        }
        self.attempts.push(now);
        let hash = digest(code.unwrap_or("").as_bytes());
        let position = self.pairings.iter().position(|entry| entry.hash == hash);
        let entry = position.map(|index| self.pairings.remove(index));
        match entry {
            Some(entry) if entry.expires >= now && entry.profile == profile => {
                enroll(&entry.name, profile, None, now, records, entropy)
            }
            _ => Err(ErrorCode::PairingRefused),
        }
    }
}

/// Crée un appareil et l'ajoute à la liste ; les appareils d'un autre profil sont écartés, comme dans Node.
///
/// Paramètres : le nom, le profil, la clé publique d'approbateur déjà validée, l'heure, la liste actuelle, l'aléa.
/// Retour : la nouvelle liste et le jeton ; un appareil avec clé d'approbateur autorise l'enrôlement Hello.
/// # Errors
///
/// `device-limit` si 32 appareils existent déjà ; `engine-entropy` si l'aléa est indisponible.
pub fn enroll(
    name: &str,
    profile: &str,
    approver_public_key: Option<&str>,
    now: u64,
    records: &[DeviceRecord],
    entropy: &mut impl Entropy,
) -> Result<(Vec<DeviceRecord>, Enrolled)> {
    if records.len() >= MAX_DEVICES {
        return Err(ErrorCode::DeviceLimit);
    }
    let issued = issue_token(entropy)?;
    let device = DeviceRecord {
        id: issued.device_id.clone(),
        name: name.to_owned(),
        profile: profile.to_owned(),
        created: iso_from_millis(now),
        hash: issued.hash,
        approver_public_key: approver_public_key.map(str::to_owned),
        hello_enrollment_allowed: approver_public_key.map(|_| true),
        hello_credential: None,
        elevation_public_key: None,
        auto_unlock: None,
        extra: serde_json::Map::new(),
    };
    let mut next: Vec<DeviceRecord> = records
        .iter()
        .filter(|value| value.profile == profile)
        .cloned()
        .collect();
    next.push(device);
    Ok((
        next,
        Enrolled {
            token: issued.token,
            device_id: issued.device_id,
        },
    ))
}

/// Enrôle un appareil nommé directement (approbation d'une demande d'association).
///
/// Paramètres : le nom, le profil courant s'il existe, la clé d'approbateur facultative, l'heure, la liste, l'aléa.
/// Retour : la nouvelle liste et le jeton.
/// # Errors
///
/// `not-configured` sans profil ; sinon les erreurs de [`enroll`].
pub fn enroll_named(
    name: &str,
    profile: Option<&str>,
    approver_public_key: Option<&str>,
    now: u64,
    records: &[DeviceRecord],
    entropy: &mut impl Entropy,
) -> Result<(Vec<DeviceRecord>, Enrolled)> {
    let profile = profile.ok_or(ErrorCode::NotConfigured)?;
    enroll(name, profile, approver_public_key, now, records, entropy)
}
