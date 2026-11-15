//! Formes JSON des demandes d'association : réponses `Opened`, `Listed`, `Polled` et cible d'approbation `Target`.

use serde::Serialize;

/// Réponse à l'ouverture d'une demande (`{action:"request"}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    /// Identifiant public de la demande (16 octets `base64url`).
    pub request_id: String,
    /// Secret du client (32 octets `base64url`), seule son empreinte est gardée.
    pub secret: String,
    /// Code de contrôle `ABCD-EFGH`.
    pub verification: String,
    /// Durée de validité en secondes.
    pub expires_in_seconds: u64,
}

/// Ligne de la liste affichée au propriétaire ; ni secret ni clé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    /// Identifiant de la demande.
    pub request_id: String,
    /// Nom proposé, nettoyé.
    pub name: String,
    /// Code de contrôle à comparer.
    pub verification: String,
    /// Une clé de PC approbateur accompagne la demande.
    pub approver: bool,
    /// Secondes restantes.
    pub expires_in_seconds: u64,
}

/// Réponse du sondage (`{action:"poll"}`), l'état choisit la forme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub enum Polled {
    /// Pas encore décidée.
    Pending {
        /// Secondes restantes.
        expires_in_seconds: u64,
    },
    /// Refusée par le propriétaire.
    Denied,
    /// Approuvée : jeton remis une seule fois.
    Approved {
        /// Jeton de l'appareil.
        token: String,
        /// Identifiant de l'appareil.
        device_id: String,
        /// Coffre qui a reçu l'appareil.
        vault_id: String,
    },
}

/// Nom et clé à enrôler pour une demande encore en attente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Nom nettoyé de l'appareil.
    pub name: String,
    /// Clé publique du PC approbateur, si fournie à l'ouverture.
    pub approver_public_key: Option<String>,
}
