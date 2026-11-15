//! Étapes pures du registre d'idempotence : ouverture, rejeu, capacité, intention et reçu. Une opération n'est
//! jamais rejouée à l'aveugle (section « Écritures » de `docs/rust/protocole.md`).

use crate::error::{ErrorCode, Result};
use crate::ledger::{CAPACITY, Ledger, LedgerEntry, Receipt, ReceiptStore, Registry};
use crate::uuid::Uuid;

/// Opération ouverte : clé du registre, empreinte de la requête et entrée précédente éventuelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedOperation {
    key: String,
    fingerprint: String,
    previous: Option<LedgerEntry>,
}

impl OpenedOperation {
    /// Clé `<profil>:<operationId>` de l'opération.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Empreinte de la requête.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Ouvre une opération dans un registre chargé.
///
/// Paramètres : le profil du coffre, l'identifiant d'opération, l'empreinte de la requête, le registre.
/// Retour : l'opération ouverte, avec l'entrée déjà présente sous la même clé.
#[must_use]
pub fn open(profile: &str, operation_id: &Uuid, fingerprint: &str, ledger: &Ledger) -> OpenedOperation {
    let key = format!("{profile}:{}", operation_id.as_str());
    let previous = ledger.get(&key).cloned();
    OpenedOperation {
        key,
        fingerprint: fingerprint.to_owned(),
        previous,
    }
}

/// Rend le reçu d'une opération déjà exécutée, marqué `replayed`.
///
/// Paramètres : le registre concerné (choix du code indéterminé), l'opération ouverte.
/// Retour : `None` si l'opération est nouvelle, sinon le reçu enregistré avec `replayed: true`.
/// # Errors
///
/// `OperationConflict` si l'empreinte diffère ; `WriteIndeterminate` ou `ShareIndeterminate` si l'intention
/// existe sans reçu.
pub fn replayed(registry: Registry, opened: &OpenedOperation) -> Result<Option<Receipt>> {
    let Some(previous) = &opened.previous else {
        return Ok(None);
    };
    if previous.fingerprint != opened.fingerprint {
        return Err(ErrorCode::OperationConflict);
    }
    let receipt = previous.receipt.as_ref().ok_or_else(|| registry.indeterminate())?;
    Ok(Some(Receipt {
        replayed: Some(true),
        ..receipt.clone()
    }))
}

/// Refuse une nouvelle opération quand le registre est plein.
///
/// # Errors
///
/// `ReceiptCapacity` si le registre compte déjà 10 000 entrées.
pub fn ensure_capacity(ledger: &Ledger) -> Result<()> {
    if ledger.len() >= CAPACITY {
        Err(ErrorCode::ReceiptCapacity)
    } else {
        Ok(())
    }
}

/// Inscrit l'intention d'écrire dans le registre en mémoire (empreinte seule, sans reçu).
pub fn record_intent(ledger: &mut Ledger, opened: &OpenedOperation) {
    ledger.insert(
        opened.key.clone(),
        LedgerEntry {
            fingerprint: opened.fingerprint.clone(),
            receipt: None,
        },
    );
}

/// Inscrit le reçu d'une écriture vérifiée dans le registre en mémoire.
pub fn record_receipt(ledger: &mut Ledger, opened: &OpenedOperation, receipt: Receipt) {
    ledger.insert(
        opened.key.clone(),
        LedgerEntry {
            fingerprint: opened.fingerprint.clone(),
            receipt: Some(receipt),
        },
    );
}

/// Inscrit l'intention puis enregistre le registre, avant toute requête d'écriture.
///
/// # Errors
///
/// L'erreur du stockage.
pub fn persist_intent<S: ReceiptStore>(
    store: &mut S,
    registry: Registry,
    ledger: &mut Ledger,
    opened: &OpenedOperation,
) -> Result<()> {
    record_intent(ledger, opened);
    store.save(registry, ledger)
}

/// Inscrit le reçu puis enregistre le registre.
///
/// Retour : le reçu conservé, à renvoyer au client.
/// # Errors
///
/// L'erreur du stockage.
pub fn persist_receipt<S: ReceiptStore>(
    store: &mut S,
    registry: Registry,
    ledger: &mut Ledger,
    opened: &OpenedOperation,
    receipt: Receipt,
) -> Result<Receipt> {
    record_receipt(ledger, opened, receipt.clone());
    store.save(registry, ledger)?;
    Ok(receipt)
}
