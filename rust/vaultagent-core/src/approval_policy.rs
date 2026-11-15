//! Politique `approval-policy.json` : Windows Hello obligatoire pour tous les appareils d'un profil une fois qu'un
//! premier appareil l'a enrôlé. La lecture et l'écriture du fichier se font ailleurs (tâche 2.2).

use serde::{Deserialize, Serialize};

/// Contenu du fichier `approval-policy.json` ; un fichier absent se lit comme la politique vide.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalPolicy {
    /// Profil du compte auquel la politique s'applique.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Hello exigé pour tous les appareils du profil.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_hello: Option<bool>,
}

impl ApprovalPolicy {
    /// Politique qui exige Hello pour tout un profil.
    ///
    /// Paramètres : le profil du compte.
    /// Retour : la politique à enregistrer.
    #[must_use]
    pub fn requiring_hello(profile: &str) -> Self {
        Self {
            profile: Some(profile.to_owned()),
            require_hello: Some(true),
        }
    }

    /// Indique si la politique exige Hello pour le profil courant.
    ///
    /// Paramètres : le profil du compte, absent si le coffre n'est pas configuré.
    /// Retour : `true` si la politique porte ce profil et `requireHello` vrai.
    #[must_use]
    pub fn requires_hello_for(&self, profile: Option<&str>) -> bool {
        self.profile.is_some() && self.profile.as_deref() == profile && self.require_hello == Some(true)
    }
}
