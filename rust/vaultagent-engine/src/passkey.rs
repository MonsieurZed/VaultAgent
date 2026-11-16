//! Passkey (identifiant FIDO2) stockée dans une fiche de connexion.
//!
//! Écart au relevé strict : `passkey-create` stocke l'identifiant complet, donc tous les champs FIDO2 y figurent,
//! pas seulement ceux que `fork/agent.ts` compare (`credentialId`, `keyValue`, `rpId`).

use serde::{Deserialize, Serialize};

use crate::secret::Secret;

/// Identifiant FIDO2 d'une fiche.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Passkey {
    /// Identifiant du credential.
    pub credential_id: String,
    /// Type de clé.
    pub key_type: String,
    /// Algorithme de la clé.
    pub key_algorithm: String,
    /// Courbe de la clé.
    pub key_curve: String,
    /// Clé privée encodée.
    pub key_value: Secret,
    /// Domaine de la partie de confiance.
    pub rp_id: String,
    /// Nom de la partie de confiance.
    pub rp_name: Option<String>,
    /// Identifiant de l'utilisateur chez la partie de confiance.
    pub user_handle: Option<String>,
    /// Nom d'utilisateur.
    pub user_name: Option<String>,
    /// Nom affiché de l'utilisateur.
    pub user_display_name: Option<String>,
    /// Compteur de signatures, en texte.
    pub counter: String,
    /// Credential découvrable (`"true"` ou `"false"`), en texte.
    pub discoverable: String,
    /// Date de création ISO.
    pub creation_date: Option<String>,
}

/// Passkey et fiche qui la porte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasskeyEntry {
    /// Identifiant de la fiche.
    pub item_id: String,
    /// Identifiant FIDO2.
    pub credential: Passkey,
}
