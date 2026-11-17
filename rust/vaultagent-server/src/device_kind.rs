//! Rôle d'un appareil tel que le serveur le connaît : agent, ou validateur avec son mode de validation.

use std::collections::HashMap;

use vaultagent_core::DeviceRecord;

/// Rôle et mode d'un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    /// Appareil sans clé de validateur.
    Agent,
    /// Validateur par carte (aucune clé d'élévation ni justificatif Hello connus).
    Card,
    /// Validateur avec une clé d'élévation enregistrée (UAC).
    Elevation,
    /// Validateur avec un justificatif Windows Hello enrôlé.
    Hello,
}

impl DeviceKind {
    /// Rôle d'un enregistrement : UAC si une clé d'élévation existe, sinon Hello si un justificatif existe, sinon
    /// carte pour un validateur ; agent sans clé de validateur.
    ///
    /// Paramètres : l'enregistrement.
    /// Retour : le rôle.
    #[must_use]
    pub fn of(record: &DeviceRecord) -> Self {
        if record.approver_public_key.is_none() {
            Self::Agent
        } else if record.elevation_public_key.is_some() {
            Self::Elevation
        } else if record.hello_credential.is_some() {
            Self::Hello
        } else {
            Self::Card
        }
    }

    /// Indique si l'appareil valide les demandes.
    ///
    /// Paramètres : aucun.
    /// Retour : `true` pour tout rôle autre qu'agent.
    #[must_use]
    pub fn is_validator(self) -> bool {
        self != Self::Agent
    }
}

/// Rôles des appareils d'un profil, par identifiant.
///
/// Paramètres : les enregistrements, le profil.
/// Retour : la table identifiant → rôle.
#[must_use]
pub fn kinds(records: &[DeviceRecord], profile: &str) -> HashMap<String, DeviceKind> {
    records
        .iter()
        .filter(|record| record.profile == profile)
        .map(|record| (record.id.clone(), DeviceKind::of(record)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Enregistrement d'essai avec des champs facultatifs.
    fn record(extra: &serde_json::Value) -> DeviceRecord {
        let mut base =
            json!({"id": "d", "name": "n", "profile": "p", "created": "2026-01-01T00:00:00.000Z", "hash": "0"});
        base.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value(base).unwrap()
    }

    #[test]
    fn the_mode_follows_the_keys_the_server_knows() {
        assert_eq!(DeviceKind::of(&record(&json!({}))), DeviceKind::Agent);
        assert_eq!(
            DeviceKind::of(&record(&json!({"approverPublicKey": "k"}))),
            DeviceKind::Card
        );
        let hello = json!({"approverPublicKey": "k", "helloCredential": {"id": "i", "keyType": "EC2", "first": "a", "second": "b"}});
        assert_eq!(DeviceKind::of(&record(&hello)), DeviceKind::Hello);
        let both = json!({"approverPublicKey": "k", "elevationPublicKey": "e"});
        assert_eq!(DeviceKind::of(&record(&both)), DeviceKind::Elevation);
        assert!(DeviceKind::Card.is_validator() && !DeviceKind::Agent.is_validator());
    }
}
