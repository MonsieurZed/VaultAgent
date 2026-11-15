//! Enregistrement d'un appareil au format exact de `devices.json`, vue publique et date ISO de création.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Identifiant WebAuthn d'un PC approbateur et coordonnées de sa clé publique.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloCredential {
    /// Identifiant du justificatif.
    pub id: String,
    /// `EC2` ou `RSA`.
    pub key_type: String,
    /// Premier composant de la clé (x ou module).
    pub first: String,
    /// Second composant de la clé (y ou exposant).
    pub second: String,
}

/// Appareil associé ; les champs facultatifs sont absents du JSON quand ils n'ont pas de valeur. Les champs
/// inconnus d'une version plus récente sont conservés tels quels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRecord {
    /// Identifiant UUID, deuxième segment du jeton.
    pub id: String,
    /// Nom affiché.
    pub name: String,
    /// Profil du compte auquel l'appareil appartient.
    pub profile: String,
    /// Date de création ISO 8601 (`toISOString`).
    pub created: String,
    /// Empreinte SHA-256 hexadécimale du jeton.
    pub hash: String,
    /// Clé publique RSA du PC approbateur (SPKI base64).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approver_public_key: Option<String>,
    /// Autorisation d'enrôler Windows Hello.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hello_enrollment_allowed: Option<bool>,
    /// Justificatif Windows Hello enrôlé.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hello_credential: Option<HelloCredential>,
    /// Clé publique RSA d'élévation (SPKI base64).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation_public_key: Option<String>,
    /// Acceptation automatique des demandes de déverrouillage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_unlock: Option<bool>,
    /// Champs inconnus conservés à la réécriture.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Appareil tel que listé à l'administrateur : jamais l'empreinte ni les clés.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicDevice {
    /// Identifiant.
    pub id: String,
    /// Nom affiché.
    pub name: String,
    /// Date de création ISO 8601.
    pub created: String,
    /// Acceptation automatique active.
    pub auto_unlock: bool,
}

impl DeviceRecord {
    /// Vue publique de l'appareil.
    ///
    /// Paramètres : aucun.
    /// Retour : identifiant, nom, date et acceptation automatique (faux si absente).
    #[must_use]
    pub fn public(&self) -> PublicDevice {
        PublicDevice {
            id: self.id.clone(),
            name: self.name.clone(),
            created: self.created.clone(),
            auto_unlock: self.auto_unlock.unwrap_or(false),
        }
    }
}

/// Formate un instant en millisecondes Unix comme `Date.prototype.toISOString` (`2026-11-15T10:20:30.123Z`).
///
/// Paramètres : les millisecondes depuis 1970 (UTC).
/// Retour : la date ISO 8601.
#[must_use]
pub fn iso_from_millis(millis: u64) -> String {
    let days = i64::try_from(millis / 86_400_000).unwrap_or(0);
    let rest = millis % 86_400_000;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1000 % 60,
        rest % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_matches_to_iso_string() {
        assert_eq!(iso_from_millis(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            iso_from_millis(951_782_400_000 + 86_399_999),
            "2000-02-29T23:59:59.999Z"
        );
        assert_eq!(iso_from_millis(1_795_000_000_123), "2026-11-18T11:06:40.123Z");
        assert_eq!(iso_from_millis(4_102_444_800_000), "2100-01-01T00:00:00.000Z");
    }

    #[test]
    fn record_round_trips_node_json_and_keeps_unknown_fields() {
        let text = r#"{"id":"a","name":"n","profile":"p","created":"c","hash":"h","approverPublicKey":"k","helloEnrollmentAllowed":false,"future":{"x":1}}"#;
        let record: DeviceRecord = serde_json::from_str(text).unwrap();
        assert_eq!(record.hello_enrollment_allowed, Some(false));
        assert_eq!(record.extra["future"]["x"], 1);
        assert_eq!(serde_json::to_string(&record).unwrap(), text);
        assert!(!record.public().auto_unlock);
    }
}
