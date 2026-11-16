//! Reprise d'un dossier `/data` écrit par le serveur Node, sans migration. Les fixtures de
//! `tests/fixtures/node-data` ont été produites par `node` (clé factice 0..31, mot de passe factice).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;

use common::{FICTITIOUS, Rig, TempDir, connectable, names};
use vaultagent_core::{ApprovalPolicy, ReceiptStore, Registry};
use vaultagent_engine::{Call, Secret};
use vaultagent_server::Store;

const FILES: [&str; 5] = [
    "machine.key",
    "account.json",
    "devices.json",
    "approval-policy.json",
    "write-receipts.json",
];

/// Copie les fixtures Node dans un dossier temporaire (clé en 0600 sous Unix).
fn node_data() -> TempDir {
    let dir = TempDir::new("node");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/node-data");
    let target = dir.path().join("data");
    std::fs::create_dir_all(&target).unwrap();
    for name in FILES {
        std::fs::copy(source.join(name), target.join(name)).unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(target.join("machine.key"), std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    dir
}

/// Contenu brut d'un fichier du magasin.
fn read(store: &Store, name: &str) -> Vec<u8> {
    std::fs::read(store.directory().join(name)).unwrap()
}

#[test]
fn node_account_is_read_and_decrypted_with_the_node_key() {
    let dir = node_data();
    let store = Store::open(&dir.path().join("data")).unwrap();
    let account = store.account().unwrap();
    assert_eq!(account.server, "https://vault.example.test");
    assert_eq!(account.timeout_minutes, Some(15));
    assert_eq!(account.profile, "11111111-1111-4111-8111-111111111111");
    assert_eq!(store.unprotect(&account.password).unwrap().as_str(), FICTITIOUS);
    assert_eq!(store.devices().len(), 1);
}

#[test]
fn rust_rewrites_node_files_byte_for_byte() {
    let dir = node_data();
    let store = Store::open(&dir.path().join("data")).unwrap();
    let account = store.account().unwrap();
    let before = FILES.map(|name| read(&store, name));
    store.save("account.json", &account).unwrap();
    store.update_devices(|current| Ok(current.to_vec())).unwrap();
    store.save_approval_policy(&store.approval_policy().unwrap()).unwrap();
    let mut access = &store;
    let ledger = ReceiptStore::load(&access, Registry::Write).unwrap();
    assert_eq!(ledger.len(), 1);
    ReceiptStore::save(&mut access, Registry::Write, &ledger).unwrap();
    assert_eq!(FILES.map(|name| read(&store, name)), before);
    assert_eq!(names(store.directory()), {
        let mut expected = FILES.to_vec();
        expected.sort_unstable();
        expected
    });
}

#[test]
fn node_policy_is_understood() {
    let dir = node_data();
    let store = Store::open(&dir.path().join("data")).unwrap();
    let policy = store.approval_policy().unwrap();
    assert_eq!(
        policy,
        ApprovalPolicy::requiring_hello("11111111-1111-4111-8111-111111111111")
    );
}

#[test]
fn session_unlocks_a_node_account_with_its_stored_password() {
    let dir = node_data();
    let store = Arc::new(Store::open(&dir.path().join("data")).unwrap());
    let rig = Rig::around(dir, store);
    let status = rig.session.status();
    assert!(status.configured && !status.unlocked);
    assert_eq!(status.timeout_minutes, Some(15));
    let handle = connectable(&rig.factory);
    rig.session.unlock(None).unwrap();
    assert_eq!(
        handle.calls(),
        [Call::Connect {
            server: "https://vault.example.test".to_owned(),
            email: "probe@example.invalid".to_owned(),
            password: Secret::from(FICTITIOUS),
            code: None,
        }]
    );
    let (_, profile) = rig.factory.starts().remove(0);
    assert_eq!(profile, "11111111-1111-4111-8111-111111111111");
}
