//! Tests des coffres : registre, un magasin par coffre, doublon de compte, verrouillage global.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{TempDir, connectable, input};
use serde_json::json;
use vaultagent_core::{ErrorCode, SeededEntropy};
use vaultagent_engine::FakeFactory;
use vaultagent_server::{ManualClock, ServerError, SharedEntropy, Store, Vaults};

/// Ouvre les coffres d'un dossier temporaire.
fn open(dir: &TempDir, factory: &FakeFactory) -> Result<Vaults, ServerError> {
    let root = Arc::new(Store::open(&dir.path().join("data"))?);
    Vaults::open(
        root,
        Arc::new(factory.clone()),
        Arc::new(ManualClock::new(common::WALL_BASE)),
        SharedEntropy::new(Box::new(SeededEntropy::new(3))),
    )
}

#[test]
fn missing_registry_gives_the_default_vault() {
    let dir = TempDir::new("vdefault");
    let vaults = open(&dir, &FakeFactory::new()).unwrap();
    let all = vaults.metadata();
    assert_eq!(all.len(), 1);
    assert_eq!(
        (all[0].id.as_str(), all[0].name.as_str()),
        ("default", "Coffre existant")
    );
    assert!(!all[0].status.configured);
    assert_eq!(vaults.get(None).unwrap().id, "default");
    assert_eq!(
        vaults.get(Some("autre")).err(),
        Some(ServerError::Code(ErrorCode::VaultMissing))
    );
}

#[test]
fn invalid_registry_is_refused() {
    let dir = TempDir::new("vbad");
    let data = dir.path().join("data");
    let seed = Store::open(&data).unwrap();
    seed.save("vaults.json", &json!([{"id": "sans-defaut", "name": "x"}]))
        .unwrap();
    assert_eq!(
        open(&dir, &FakeFactory::new()).err(),
        Some(ServerError::Code(ErrorCode::InvalidVaultRegistry))
    );
    seed.save(
        "vaults.json",
        &json!([{"id": "default", "name": "a"}, {"id": "pas-un-uuid", "name": "b"}]),
    )
    .unwrap();
    assert_eq!(
        open(&dir, &FakeFactory::new()).err(),
        Some(ServerError::Code(ErrorCode::InvalidVault))
    );
}

#[test]
fn save_name_adds_a_vault_with_its_own_store_and_renames_it() {
    let dir = TempDir::new("vadd");
    let vaults = open(&dir, &FakeFactory::new()).unwrap();
    let added = vaults.save_name(None, Some("  Famille  ")).unwrap();
    assert_eq!(added.name, "Famille");
    let folder = dir.path().join("data").join("vaults").join(&added.id);
    assert!(folder.join("machine.key").exists());
    let registry: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("data").join("vaults.json")).unwrap()).unwrap();
    assert_eq!(
        registry,
        json!([
            {"id": "default", "name": "Coffre existant"},
            {"id": added.id, "name": "Famille"}
        ])
    );
    let renamed = vaults.save_name(Some(&added.id), Some("Amis")).unwrap();
    assert_eq!(renamed.name, "Amis");
    let reopened = open(&dir, &FakeFactory::new()).unwrap();
    assert_eq!(reopened.metadata().len(), 2);
    assert_eq!(reopened.get(Some(&added.id)).unwrap().name(), "Amis");
    assert_eq!(
        vaults.save_name(None, Some("  ")).err(),
        Some(ServerError::Code(ErrorCode::InvalidVaultName))
    );
}

#[test]
fn vault_limit_is_enforced() {
    let dir = TempDir::new("vlimit");
    let vaults = open(&dir, &FakeFactory::new()).unwrap();
    for index in 1..8 {
        vaults.save_name(None, Some(&format!("Coffre {index}"))).unwrap();
    }
    assert_eq!(
        vaults.save_name(None, Some("Neuvième")).err(),
        Some(ServerError::Code(ErrorCode::VaultLimit))
    );
    assert_eq!(vaults.list().len(), 8);
}

#[test]
fn same_account_cannot_be_configured_in_two_vaults() {
    let dir = TempDir::new("vdup");
    let factory = FakeFactory::new();
    let vaults = open(&dir, &factory).unwrap();
    let second = vaults.save_name(None, Some("Second")).unwrap();
    connectable(&factory);
    let default = vaults.get(None).unwrap();
    vaults.configure(&default, &input(json!(15))).unwrap();
    let other = vaults.get(Some(&second.id)).unwrap();
    let mut duplicate = input(json!(15));
    duplicate.email = common::EMAIL.to_uppercase();
    assert_eq!(
        vaults.configure(&other, &duplicate).err(),
        Some(ServerError::Code(ErrorCode::DuplicateAccount))
    );
    duplicate.email = "distinct@example.invalid".to_owned();
    connectable(&factory);
    vaults.configure(&other, &duplicate).unwrap();
    assert!(other.store.account().is_some());
    assert_ne!(other.store.directory(), default.store.directory());
}

#[test]
fn lock_and_expire_apply_to_every_vault() {
    let dir = TempDir::new("vlock");
    let factory = FakeFactory::new();
    let vaults = open(&dir, &factory).unwrap();
    let second = vaults.save_name(None, Some("Second")).unwrap();
    let default = vaults.get(None).unwrap();
    let other = vaults.get(Some(&second.id)).unwrap();
    connectable(&factory);
    vaults.configure(&default, &input(json!(15))).unwrap();
    let mut distinct = input(json!(15));
    distinct.email = "distinct@example.invalid".to_owned();
    connectable(&factory);
    vaults.configure(&other, &distinct).unwrap();
    vaults.expire();
    assert!(vaults.metadata().iter().all(|vault| vault.status.unlocked));
    vaults.lock();
    assert!(vaults.metadata().iter().all(|vault| !vault.status.unlocked));
}
