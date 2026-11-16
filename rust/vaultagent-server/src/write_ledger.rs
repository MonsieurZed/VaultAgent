//! Registre d'idempotence d'une opération agent (écriture ou partage) : ouverture, rejeu, intention, reçu.

use serde_json::Value;
use vaultagent_core::ledger_flow::{OpenedOperation, ensure_capacity, open, persist_intent, persist_receipt, replayed};
use vaultagent_core::{Ledger, Receipt, Registry, Uuid};

use crate::error::Result;
use crate::store::Store;

/// Opération ouverte dans un registre chargé.
pub struct Operation<'a> {
    store: &'a Store,
    registry: Registry,
    ledger: Ledger,
    opened: OpenedOperation,
}

impl<'a> Operation<'a> {
    /// Charge le registre et ouvre l'opération.
    ///
    /// Paramètres : le magasin, le registre, le profil du compte, l'identifiant d'opération, la requête sans
    /// `vaultId`.
    /// Retour : l'opération ouverte et, si elle a déjà été exécutée, son reçu marqué `replayed`.
    ///
    /// # Errors
    ///
    /// `operation-conflict`, `write-indeterminate` ou `share-indeterminate`, ou une erreur de lecture du registre.
    pub fn open(
        store: &'a Store,
        registry: Registry,
        profile: &str,
        operation_id: &Uuid,
        request: &Value,
    ) -> Result<(Self, Option<Receipt>)> {
        let ledger: Ledger = store.load(registry.file_name(), Ledger::new())?;
        let opened = open(profile, operation_id, &store.fingerprint(request)?, &ledger);
        let replay = replayed(registry, &opened)?;
        Ok((
            Self {
                store,
                registry,
                ledger,
                opened,
            },
            replay,
        ))
    }

    /// Refuse l'opération si le registre est plein.
    ///
    /// Paramètres : aucun.
    ///
    /// # Errors
    ///
    /// `receipt-capacity`.
    pub fn ensure_capacity(&self) -> Result<()> {
        Ok(ensure_capacity(&self.ledger)?)
    }

    /// Enregistre l'intention d'écrire, avant toute requête d'écriture.
    ///
    /// Paramètres : aucun.
    ///
    /// # Errors
    ///
    /// Erreur d'écriture du registre.
    pub fn intent(&mut self) -> Result<()> {
        let mut handle = self.store;
        Ok(persist_intent(
            &mut handle,
            self.registry,
            &mut self.ledger,
            &self.opened,
        )?)
    }

    /// Enregistre le reçu de l'opération vérifiée.
    ///
    /// Paramètres : le reçu.
    /// Retour : le reçu conservé.
    ///
    /// # Errors
    ///
    /// Erreur d'écriture du registre.
    pub fn receipt(&mut self, receipt: Receipt) -> Result<Receipt> {
        let mut handle = self.store;
        Ok(persist_receipt(
            &mut handle,
            self.registry,
            &mut self.ledger,
            &self.opened,
            receipt,
        )?)
    }
}
