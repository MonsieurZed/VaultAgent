//! Compte Bitwarden d'un coffre, au format exact de `account.json` écrit par le serveur Node.

use serde::{Deserialize, Serialize};
use vaultagent_core::Protected;

/// Contenu de `account.json` ; l'ordre des champs est celui de `JSON.stringify` côté Node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Origine du serveur Bitwarden.
    pub server: String,
    /// Courriel en minuscules.
    pub email: String,
    /// Délai d'inactivité en minutes, `null` s'il est désactivé.
    pub timeout_minutes: Option<u32>,
    /// Identifiant du profil du moteur.
    pub profile: String,
    /// Mot de passe maître chiffré par la clé machine.
    pub password: Protected,
}
