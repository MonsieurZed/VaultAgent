//! Partage : registre d'idempotence `share-receipts.json` (rejeu, conflit, intention sans reçu) et cas doré.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::ledger_flow::{ensure_capacity, persist_intent, persist_receipt};
use vaultagent_core::share_flow::begin;
use vaultagent_core::share_verify::receipt;
use vaultagent_core::{ErrorCode, Ledger, ReceiptStore, Registry, ShareAction, ShareRequest, ShareStart, Uuid};

const ORG: &str = "10000000-0000-4000-8000-000000000001";
const COLLECTION: &str = "20000000-0000-4000-8000-000000000001";
const OTHER: &str = "20000000-0000-4000-8000-000000000002";
const ITEM: &str = "00000000-0000-4000-8000-000000000001";
const OPERATION: &str = "00000000-0000-4000-8000-000000000002";
const KEY: [u8; 32] = [9; 32];

/// Requête `apply` de référence.
fn apply() -> Value {
    json!({"action": "apply", "itemId": ITEM, "organizationId": ORG, "collectionIds": [COLLECTION],
           "operationId": OPERATION, "expectedRevisionDate": "2026-11-08T00:00:00.000Z",
           "confirmOwnershipTransfer": true})
}

/// Valide une requête.
fn parse(raw: Value) -> Result<ShareRequest, ErrorCode> {
    ShareRequest::parse(&raw)
}

/// Stockage de registre en mémoire.
#[derive(Default)]
struct Memory(Ledger);

impl ReceiptStore for Memory {
    /// Rend le registre.
    fn load(&self, _: Registry) -> vaultagent_core::Result<Ledger> {
        Ok(self.0.clone())
    }

    /// Remplace le registre.
    fn save(&mut self, _: Registry, ledger: &Ledger) -> vaultagent_core::Result<()> {
        self.0 = ledger.clone();
        Ok(())
    }
}

#[test]
fn share_ledger_replays_conflicts_and_flags_indeterminate() {
    let request = parse(apply()).unwrap();
    let mut store = Memory::default();
    let mut ledger = store.load(Registry::Share).unwrap();
    let preview =
        parse(json!({"action": "preview", "itemId": ITEM, "organizationId": ORG, "collectionIds": [COLLECTION]}))
            .unwrap();
    assert_eq!(begin(&KEY, "default", &preview, &ledger).unwrap(), ShareStart::Preview);

    let ShareStart::Fresh(opened) = begin(&KEY, "default", &request, &ledger).unwrap() else {
        panic!("opération neuve attendue");
    };
    ensure_capacity(&ledger).unwrap();
    persist_intent(&mut store, Registry::Share, &mut ledger, &opened).unwrap();
    assert_eq!(
        begin(&KEY, "default", &request, &ledger).unwrap_err(),
        ErrorCode::ShareIndeterminate
    );

    let made = receipt(&request, &Uuid::parse(ITEM).unwrap()).unwrap();
    persist_receipt(&mut store, Registry::Share, &mut ledger, &opened, made.clone()).unwrap();
    let ShareStart::Replayed(again) = begin(&KEY, "default", &request, &store.0).unwrap() else {
        panic!("rejeu attendu");
    };
    assert_eq!(again.replayed, Some(true));
    assert_eq!(again.id, made.id);

    let mut changed = apply();
    changed["collectionIds"] = json!([OTHER]);
    let other = parse(changed).unwrap();
    assert_eq!(
        begin(&KEY, "default", &other, &store.0).unwrap_err(),
        ErrorCode::OperationConflict
    );
    let mut unconfirmed = apply();
    unconfirmed.as_object_mut().unwrap().remove("confirmOwnershipTransfer");
    let different = parse(unconfirmed).unwrap();
    assert_eq!(
        begin(&KEY, "default", &different, &store.0).unwrap_err(),
        ErrorCode::OperationConflict
    );
    assert!(matches!(
        begin(&KEY, "autre", &request, &store.0).unwrap(),
        ShareStart::Fresh(_)
    ));
}

#[test]
fn golden_share_locked_case_is_a_valid_preview() {
    let source = include_str!("../../contrat/share-locked.json").replace("{{itemId}}", ITEM);
    let case: Value = serde_json::from_str(&source).unwrap();
    let request = ShareRequest::parse(&case["request"]["body"]).unwrap();
    assert_eq!(request.action(), ShareAction::Preview);
    assert_eq!(request.item_id().unwrap().as_str(), ITEM);
    assert_eq!(
        ErrorCode::parse(case["response"]["body"]["error"].as_str().unwrap()),
        ErrorCode::Locked
    );
    assert_eq!(
        u64::from(ErrorCode::Locked.http_status()),
        case["response"]["status"].as_u64().unwrap()
    );
}
