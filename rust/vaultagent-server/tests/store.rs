//! Tests du magasin fichier : clé machine, écriture atomique, appareils sérialisés, registres, permissions.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{TempDir, names};
use serde_json::{Value, json};
use vaultagent_core::{ApprovalPolicy, DeviceRecord, Ledger, LedgerEntry, ReceiptStore, Registry, SeededEntropy};
use vaultagent_server::{ServerError, Store};

/// Appareil factice d'identifiant donné.
fn device(id: &str) -> DeviceRecord {
    serde_json::from_value(json!({
        "id": id, "name": "Client factice", "profile": "p", "created": "2026-11-01T10:00:00.000Z", "hash": "0"
    }))
    .unwrap()
}

#[test]
fn key_is_created_with_32_bytes_and_kept_on_reopen() {
    let dir = TempDir::new("key");
    let data = dir.path().join("data");
    Store::open(&data).unwrap();
    let first = std::fs::read(data.join("machine.key")).unwrap();
    assert_eq!(first.len(), 32);
    Store::open(&data).unwrap();
    assert_eq!(std::fs::read(data.join("machine.key")).unwrap(), first);
}

#[test]
fn key_of_wrong_size_is_refused() {
    let dir = TempDir::new("badkey");
    std::fs::write(dir.path().join("machine.key"), [1u8; 16]).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path().join("machine.key"), std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert_eq!(Store::open(dir.path()).err(), Some(ServerError::PrivateKeyInvalid));
}

#[test]
fn load_uses_fallback_when_absent_and_fails_on_bad_json() {
    let dir = TempDir::new("load");
    let store = Store::open(&dir.path().join("data")).unwrap();
    assert_eq!(store.load("missing.json", json!([1])).unwrap(), json!([1]));
    std::fs::write(store.directory().join("broken.json"), "{not json").unwrap();
    assert!(matches!(
        store.load("broken.json", Value::Null),
        Err(ServerError::Json(_))
    ));
}

#[test]
fn save_is_atomic_and_leaves_no_temporary_file() {
    let dir = TempDir::new("atomic");
    let store = Store::open(&dir.path().join("data")).unwrap();
    for round in 0..20 {
        store.save("state.json", &json!({"round": round})).unwrap();
    }
    assert_eq!(store.load("state.json", Value::Null).unwrap(), json!({"round": 19}));
    assert_eq!(names(store.directory()), ["machine.key", "state.json"]);
    assert_eq!(
        std::fs::read_to_string(store.directory().join("state.json")).unwrap(),
        r#"{"round":19}"#
    );
}

#[test]
fn failed_save_removes_its_temporary_file() {
    let dir = TempDir::new("failsave");
    let store = Store::open(&dir.path().join("data")).unwrap();
    std::fs::create_dir(store.directory().join("target.json")).unwrap();
    assert!(store.save("target.json", &json!(1)).is_err());
    assert_eq!(names(store.directory()), ["machine.key", "target.json"]);
}

#[test]
fn protect_round_trips_and_differs_per_call() {
    let dir = TempDir::new("protect");
    let store = Store::open(&dir.path().join("data")).unwrap();
    let mut entropy = SeededEntropy::new(1);
    let first = store.protect("secret factice", &mut entropy).unwrap();
    let second = store.protect("secret factice", &mut entropy).unwrap();
    assert_ne!(first, second);
    assert_eq!(store.unprotect(&first).unwrap().as_str(), "secret factice");
    let other = Store::open(&dir.path().join("other")).unwrap();
    assert!(other.unprotect(&first).is_err());
}

#[test]
fn device_updates_are_serialized_and_persisted() {
    let dir = TempDir::new("devices");
    let store = Arc::new(Store::open(&dir.path().join("data")).unwrap());
    let workers: Vec<_> = (0..8)
        .map(|index| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                store
                    .update_devices(|current| {
                        let mut next = current.to_vec();
                        next.push(device(&format!("id-{index}")));
                        Ok(next)
                    })
                    .unwrap();
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(store.devices().len(), 8);
    let reopened = Store::open(store.directory()).unwrap();
    assert_eq!(reopened.devices().len(), 8);
}

#[test]
fn failed_device_update_keeps_the_list() {
    let dir = TempDir::new("devfail");
    let store = Store::open(&dir.path().join("data")).unwrap();
    store.update_devices(|_| Ok(vec![device("a")])).unwrap();
    let refused = store.update_devices(|_| Err(ServerError::Cancelled));
    assert_eq!(refused.err(), Some(ServerError::Cancelled));
    assert_eq!(store.devices().len(), 1);
}

#[test]
fn receipt_store_round_trips_each_registry() {
    let dir = TempDir::new("ledger");
    let store = Store::open(&dir.path().join("data")).unwrap();
    let mut access = &store;
    assert!(ReceiptStore::load(&access, Registry::Write).unwrap().is_empty());
    let mut ledger = Ledger::new();
    ledger.insert(
        "profile:op".to_owned(),
        LedgerEntry {
            fingerprint: "ab".repeat(32),
            receipt: None,
        },
    );
    ReceiptStore::save(&mut access, Registry::Share, &ledger).unwrap();
    assert_eq!(ReceiptStore::load(&access, Registry::Share).unwrap(), ledger);
    assert!(ReceiptStore::load(&access, Registry::Write).unwrap().is_empty());
    assert!(store.directory().join("share-receipts.json").exists());
}

#[test]
fn approval_policy_round_trips() {
    let dir = TempDir::new("policy");
    let store = Store::open(&dir.path().join("data")).unwrap();
    assert_eq!(store.approval_policy().unwrap(), ApprovalPolicy::default());
    let policy = ApprovalPolicy::requiring_hello("profil");
    store.save_approval_policy(&policy).unwrap();
    assert_eq!(store.approval_policy().unwrap(), policy);
}

#[cfg(unix)]
#[test]
fn unix_modes_are_private_and_loose_key_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let dir = TempDir::new("modes");
    let data = dir.path().join("data");
    let store = Store::open(&data).unwrap();
    store.save("state.json", &json!(1)).unwrap();
    assert_eq!(mode(&data), 0o700);
    assert_eq!(mode(&data.join("machine.key")), 0o600);
    assert_eq!(mode(&data.join("state.json")), 0o600);
    std::fs::set_permissions(data.join("machine.key"), std::fs::Permissions::from_mode(0o640)).unwrap();
    assert_eq!(Store::open(&data).err(), Some(ServerError::PrivateKeyPermissions));
}
