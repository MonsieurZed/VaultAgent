//! Corps et réponses JSON de `POST /api/pair` pour l'association par approbation web.

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// Corps d'une requête d'association.
#[derive(Serialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum PairBody<'a> {
    /// Ouverture d'une demande, nommée.
    Request {
        /// Nom de l'appareil.
        name: &'a str,
        /// Clé publique d'approbateur (SPKI base64), absente sans `--approver`.
        #[serde(rename = "approverPublicKey", skip_serializing_if = "Option::is_none")]
        approver_public_key: Option<&'a str>,
    },
    /// Interrogation d'une demande ouverte.
    Poll {
        /// Identifiant de la demande.
        #[serde(rename = "requestId")]
        request_id: &'a str,
        /// Secret de la demande.
        secret: &'a str,
    },
}

/// Réponse à l'ouverture d'une demande.
#[derive(Deserialize)]
pub struct Opened {
    /// Identifiant de la demande.
    #[serde(rename = "requestId")]
    pub request_id: String,
    /// Secret de la demande, effacé à la destruction.
    pub secret: String,
    /// Code de vérification à comparer avec l'écran de l'administrateur.
    pub verification: String,
    /// Durée de validité restante.
    #[serde(rename = "expiresInSeconds")]
    pub expires_in_seconds: u64,
}

impl Drop for Opened {
    /// Efface le secret de la demande.
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

/// Réponse à une interrogation.
#[derive(Deserialize)]
pub struct Polled {
    /// `pending`, `denied` ou `approved`.
    pub state: String,
    /// Jeton remis une seule fois à l'approbation, effacé à la destruction.
    pub token: Option<String>,
    /// Coffre associé.
    #[serde(rename = "vaultId")]
    pub vault_id: Option<String>,
}

impl Drop for Polled {
    /// Efface le jeton reçu.
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

/// Sortie JSON de `pair-request` en cas de succès.
#[derive(Serialize)]
pub struct PairedOutput {
    /// Toujours vrai.
    pub paired: bool,
    /// Coffre associé, si le serveur l'a donné.
    #[serde(rename = "vaultId", skip_serializing_if = "Option::is_none")]
    pub vault_id: Option<String>,
}
