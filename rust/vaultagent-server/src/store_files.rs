//! Fichiers dérivés du magasin : appareils sérialisés, politique d'approbation, registres d'idempotence.

use vaultagent_core::{ApprovalPolicy, DeviceRecord, ErrorCode, Ledger, ReceiptStore, Registry};

use crate::error::Result;
use crate::store::Store;
use crate::sync::locked;

const POLICY_FILE: &str = "approval-policy.json";

impl Store {
    /// Met à jour la liste des appareils : les mises à jour sont sérialisées, `devices.json` est écrit avant que la
    /// liste en mémoire ne change.
    ///
    /// Paramètres : la transformation, qui reçoit la liste courante et rend la nouvelle (ou une erreur qui annule).
    /// Retour : la nouvelle liste.
    ///
    /// # Errors
    ///
    /// L'erreur de la transformation ou de l'écriture ; la liste en mémoire est alors inchangée.
    pub fn update_devices(
        &self,
        transform: impl FnOnce(&[DeviceRecord]) -> Result<Vec<DeviceRecord>>,
    ) -> Result<Vec<DeviceRecord>> {
        let serial = locked(&self.device_writes);
        let next = transform(&self.devices())?;
        self.save("devices.json", &next)?;
        locked(&self.devices).clone_from(&next);
        drop(serial);
        Ok(next)
    }

    /// Lit `approval-policy.json` ; la politique vide s'il n'existe pas.
    ///
    /// Paramètres : aucun.
    /// Retour : la politique.
    ///
    /// # Errors
    ///
    /// Erreur de lecture ou JSON invalide.
    pub fn approval_policy(&self) -> Result<ApprovalPolicy> {
        self.load(POLICY_FILE, ApprovalPolicy::default())
    }

    /// Écrit `approval-policy.json`.
    ///
    /// Paramètres : la politique.
    /// Retour : rien.
    ///
    /// # Errors
    ///
    /// Erreur d'écriture.
    pub fn save_approval_policy(&self, policy: &ApprovalPolicy) -> Result<()> {
        self.save(POLICY_FILE, policy)
    }
}

impl ReceiptStore for &Store {
    /// Lit un registre ; vide s'il n'existe pas. Toute erreur devient `request-refused`, comme côté Node.
    fn load(&self, registry: Registry) -> vaultagent_core::Result<Ledger> {
        Store::load(self, registry.file_name(), Ledger::new()).map_err(|_| ErrorCode::RequestRefused)
    }

    /// Écrit un registre en entier, de façon atomique.
    fn save(&mut self, registry: Registry, ledger: &Ledger) -> vaultagent_core::Result<()> {
        Store::save(self, registry.file_name(), ledger).map_err(|_| ErrorCode::RequestRefused)
    }
}

impl Store {
    /// Empreinte HMAC-SHA-256 d'une requête sous forme canonique, avec la clé machine (idempotence des écritures).
    ///
    /// Paramètres : la requête JSON, `vaultId` déjà retiré.
    /// Retour : l'empreinte hexadécimale.
    ///
    /// # Errors
    ///
    /// `private-key-invalid` si la clé machine n'a pas 32 octets (impossible après `Store::open`).
    pub fn fingerprint(&self, request: &serde_json::Value) -> Result<String> {
        let key: &[u8; 32] = self
            .key
            .as_slice()
            .try_into()
            .map_err(|_| crate::error::ServerError::PrivateKeyInvalid)?;
        Ok(vaultagent_core::fingerprint(key, request))
    }
}
