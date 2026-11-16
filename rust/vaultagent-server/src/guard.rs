//! Garde d'une opération agent : équivalent de `authorize(); session.access(generation)` de Node. Elle relève la
//! génération au départ, puis vérifie après chaque appel que le client est toujours autorisé et que le coffre n'a
//! pas été verrouillé entre-temps.

use std::sync::Arc;

use vaultagent_core::device_list::authenticate;
use vaultagent_core::{ErrorCode, Uuid};
use vaultagent_engine::Engine;

use crate::engine_slot::EngineSlot;
use crate::error::Result;
use crate::vaults::VaultContext;

/// Garde d'une opération sur un coffre, pour un client authentifié.
pub struct Guard<'a> {
    vault: &'a VaultContext,
    header: Option<&'a str>,
    generation: u64,
    owner: bool,
}

impl<'a> Guard<'a> {
    /// Ouvre la garde : contrôle le client puis relève la génération de la session.
    ///
    /// Paramètres : le coffre, l'en-tête `authorization`.
    /// Retour : la garde.
    ///
    /// # Errors
    ///
    /// `unauthorized` si le client n'est plus reconnu.
    pub fn begin(vault: &'a VaultContext, header: Option<&'a str>) -> Result<Self> {
        let guard = Self {
            vault,
            header,
            generation: vault.session.generation(),
            owner: false,
        };
        guard.authorize()?;
        Ok(guard)
    }

    /// Ouvre la garde du propriétaire (routes `/api/admin/*`) : aucun jeton, le contrôle est celui d'Authentik.
    ///
    /// Paramètres : le coffre.
    /// Retour : la garde, qui relève la génération courante de la session.
    #[must_use]
    pub fn owner(vault: &'a VaultContext) -> Self {
        Self {
            vault,
            header: None,
            generation: vault.session.generation(),
            owner: true,
        }
    }

    /// Vérifie que le jeton authentifie encore un appareil du profil courant.
    ///
    /// Paramètres : aucun.
    /// Retour : l'identifiant de l'appareil.
    ///
    /// # Errors
    ///
    /// `unauthorized`.
    pub fn authorize(&self) -> Result<String> {
        if self.owner {
            return Ok(String::new());
        }
        let profile = self.vault.store.account().map(|account| account.profile);
        let profile = profile.ok_or(ErrorCode::Unauthorized)?;
        Ok(authenticate(self.header, &self.vault.store.devices(), &profile)?)
    }

    /// Autorise puis rend le moteur de la génération d'origine.
    ///
    /// Paramètres : aucun.
    /// Retour : l'emplacement du moteur.
    ///
    /// # Errors
    ///
    /// `unauthorized` ; `locked` si la session est verrouillée ou si la génération a changé.
    pub fn slot(&self) -> Result<Arc<EngineSlot>> {
        self.authorize()?;
        self.vault.session.access(self.generation)
    }

    /// Autorise et contrôle la génération, sans moteur.
    ///
    /// Paramètres : aucun.
    ///
    /// # Errors
    ///
    /// `unauthorized` ou `locked`.
    pub fn check(&self) -> Result<()> {
        self.slot().map(|_| ())
    }

    /// Appelle le moteur, puis contrôle que le client et la session sont toujours valides.
    ///
    /// Paramètres : l'opération sur le moteur.
    /// Retour : son résultat.
    ///
    /// # Errors
    ///
    /// `locked`, `unauthorized`, ou l'erreur du moteur.
    pub fn call<T>(&self, operation: impl FnOnce(&mut dyn Engine) -> vaultagent_core::Result<T>) -> Result<T> {
        let slot = self.slot()?;
        let result = slot.run(operation)?;
        self.check()?;
        Ok(result)
    }

    /// Resynchronise le coffre (`POST /sync` de Node).
    ///
    /// Paramètres : aucun.
    ///
    /// # Errors
    ///
    /// Erreurs de [`Guard::call`].
    pub fn sync(&self) -> Result<()> {
        self.call(|engine| engine.sync())
    }

    /// Lit une fiche par identifiant.
    ///
    /// Paramètres : l'identifiant.
    /// Retour : la fiche vue comme JSON par `vaultagent-core`.
    ///
    /// # Errors
    ///
    /// Erreurs de [`Guard::call`].
    pub fn item_value(&self, id: &Uuid) -> Result<serde_json::Value> {
        let item = self.call(|engine| engine.item(id))?;
        crate::item_value::item_value(&item)
    }

    /// Repousse le délai d'inactivité de la session.
    ///
    /// Paramètres : aucun.
    pub fn touch(&self) {
        self.vault.session.touch();
    }
}
