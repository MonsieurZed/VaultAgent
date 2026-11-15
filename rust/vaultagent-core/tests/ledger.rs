//! Registre d'idempotence : rejeu, conflit, intention sans reçu, capacité, enregistrement et absence de secret.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::ledger_flow::{
    OpenedOperation, ensure_capacity, open, persist_intent, persist_receipt, record_intent, record_receipt, replayed,
};
use vaultagent_core::{
    CAPACITY, ErrorCode, Ledger, LedgerEntry, Receipt, ReceiptStore, Registry, Uuid, WriteRequest, fingerprint,
};

const OP: &str = "00000000-0000-4000-8000-000000000002";
const ITEM: &str = "00000000-0000-4000-8000-000000000001";

/// Stockage en mémoire qui compte les enregistrements.
#[derive(Default)]
struct Memory {
    ledger: Ledger,
    saves: usize,
}

impl ReceiptStore for Memory {
    /// Rend le registre en mémoire.
    fn load(&self, _: Registry) -> vaultagent_core::Result<Ledger> {
        Ok(self.ledger.clone())
    }

    /// Remplace le registre en mémoire.
    fn save(&mut self, _: Registry, ledger: &Ledger) -> vaultagent_core::Result<()> {
        self.ledger = ledger.clone();
        self.saves += 1;
        Ok(())
    }
}

/// Opération ouverte pour une empreinte donnée.
fn opened(ledger: &Ledger, print: &str) -> OpenedOperation {
    open("default", &Uuid::parse(OP).unwrap(), print, ledger)
}

/// Reçu de référence.
fn receipt() -> Receipt {
    Receipt::verified(ITEM, OP, "update", Some("2026-11-08T00:00:01.000Z"))
}

#[test]
fn key_is_profile_and_operation() {
    let operation = opened(&Ledger::new(), "abc");
    assert_eq!(operation.key(), format!("default:{OP}"));
    assert_eq!(operation.fingerprint(), "abc");
}

#[test]
fn new_operation_is_not_replayed() {
    let operation = opened(&Ledger::new(), "abc");
    assert_eq!(replayed(Registry::Write, &operation), Ok(None));
}

#[test]
fn same_fingerprint_replays_the_receipt() {
    let mut ledger = Ledger::new();
    let operation = opened(&ledger, "abc");
    record_receipt(&mut ledger, &operation, receipt());
    let replay = replayed(Registry::Write, &opened(&ledger, "abc")).unwrap().unwrap();
    assert_eq!(replay.replayed, Some(true));
    assert_eq!(replay.id, ITEM);
    assert_eq!(
        serde_json::to_value(&replay).unwrap(),
        json!({"id": ITEM, "operationId": OP, "action": "update", "verified": true,
               "revisionDate": "2026-11-08T00:00:01.000Z", "replayed": true})
    );
}

#[test]
fn different_fingerprint_is_a_conflict() {
    let mut ledger = Ledger::new();
    let operation = opened(&ledger, "abc");
    record_receipt(&mut ledger, &operation, receipt());
    assert_eq!(
        replayed(Registry::Write, &opened(&ledger, "other")),
        Err(ErrorCode::OperationConflict)
    );
    let operation = opened(&ledger, "abc");
    record_intent(&mut ledger, &operation);
    assert_eq!(
        replayed(Registry::Share, &opened(&ledger, "other")),
        Err(ErrorCode::OperationConflict)
    );
}

#[test]
fn intent_without_receipt_is_indeterminate_per_registry() {
    let mut ledger = Ledger::new();
    let operation = opened(&ledger, "abc");
    record_intent(&mut ledger, &operation);
    let operation = opened(&ledger, "abc");
    assert_eq!(
        replayed(Registry::Write, &operation),
        Err(ErrorCode::WriteIndeterminate)
    );
    assert_eq!(
        replayed(Registry::Share, &operation),
        Err(ErrorCode::ShareIndeterminate)
    );
}

#[test]
fn capacity_is_ten_thousand_entries() {
    assert_eq!(CAPACITY, 10_000);
    let mut ledger = Ledger::new();
    for index in 0..CAPACITY - 1 {
        ledger.insert(
            format!("p:{index}"),
            LedgerEntry {
                fingerprint: String::new(),
                receipt: None,
            },
        );
    }
    assert_eq!(ensure_capacity(&ledger), Ok(()));
    ledger.insert(
        "p:last".to_owned(),
        LedgerEntry {
            fingerprint: String::new(),
            receipt: None,
        },
    );
    assert_eq!(ensure_capacity(&ledger), Err(ErrorCode::ReceiptCapacity));
}

#[test]
fn intent_then_receipt_are_persisted_in_order() {
    let mut store = Memory::default();
    let mut ledger = store.load(Registry::Write).unwrap();
    let operation = opened(&ledger, "abc");
    persist_intent(&mut store, Registry::Write, &mut ledger, &operation).unwrap();
    assert_eq!(store.saves, 1);
    assert_eq!(store.ledger[operation.key()].receipt, None);
    assert_eq!(store.ledger[operation.key()].fingerprint, "abc");
    let kept = persist_receipt(&mut store, Registry::Write, &mut ledger, &operation, receipt()).unwrap();
    assert_eq!(store.saves, 2);
    assert_eq!(store.ledger[operation.key()].receipt, Some(kept));
}

#[test]
fn receipt_json_carries_only_protocol_fields_and_no_secret() {
    let canary = "Fictif-SECRET-1";
    let body: Value = json!({"operationId": OP, "action": "update", "itemId": ITEM,
        "expectedRevisionDate": "2026-11-08T00:00:00.000Z", "patch": {"password": canary}});
    let request = WriteRequest::parse(&body).unwrap();
    let print = fingerprint(&[7; 32], request.raw());
    let mut ledger = Ledger::new();
    let operation = opened(&ledger, &print);
    record_intent(&mut ledger, &operation);
    record_receipt(&mut ledger, &operation, receipt());
    let text = serde_json::to_string(&ledger).unwrap();
    assert!(!text.contains(canary));
    assert!(!print.contains(canary));
    let stored: Value = serde_json::from_str(&text).unwrap();
    let keys: Vec<&String> = stored[operation.key()]["receipt"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["action", "id", "operationId", "revisionDate", "verified"]);
}
