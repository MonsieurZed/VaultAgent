//! Tests du faux moteur : réponses dans l'ordre, formes vérifiées, journal des appels, moteur arrêté, fabrique.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use serde_json::json;
use vaultagent_core::{ErrorCode, RevisionDate, Uuid};
use vaultagent_engine::{
    Account, Call, Engine, EngineFactory, FakeFactory, FakeHandle, Folder, Reply, Secret, fail, stage_error,
};
use zeroize::Zeroizing;

const ID: &str = "11111111-1111-4111-8111-111111111111";
const ORG: &str = "22222222-2222-4222-8222-222222222222";

/// Identifiant valide des tests.
fn uuid(text: &str) -> Uuid {
    Uuid::parse(text).unwrap()
}

/// Texte du code d'une erreur.
fn code<T>(result: vaultagent_core::Result<T>) -> Option<String> {
    result.err().map(|error| error.to_string())
}

#[test]
fn replies_come_in_order_then_refused() {
    let handle = FakeHandle::new();
    handle.push_ok().push(Reply::Folders(vec![Folder {
        id: "f".into(),
        name: "n".into(),
    }]));
    let mut engine = handle.engine();
    assert!(engine.sync().is_ok());
    assert_eq!(engine.folders().unwrap()[0].name, "n");
    assert_eq!(code(engine.sync()), Some("engine-refused".into()));
    assert_eq!(handle.call_names(), ["sync", "folders", "sync"]);
}

#[test]
fn scripted_failure_keeps_its_stage_code_and_a_wrong_shape_is_refused() {
    let handle = FakeHandle::new();
    handle
        .push_error(stage_error("write", "rejected"))
        .push(Reply::Text("id".into()));
    let mut engine = handle.engine();
    assert_eq!(code(engine.trash_item(&uuid(ID))), Some("write:rejected".into()));
    assert_eq!(code(engine.sync()), Some("engine-refused".into()));
    assert_eq!(fail("engine-timeout"), ErrorCode::Engine("engine-timeout".into()));
}

#[test]
fn journal_records_arguments_and_hides_secrets_in_debug() {
    let handle = FakeHandle::new();
    handle.push_ok().push(Reply::Item(Box::default())).push_ok();
    let mut engine = handle.engine();
    let password = Zeroizing::new("fake-password".to_owned());
    let second = Zeroizing::new("123456".to_owned());
    let account = Account {
        server: "https://vault.test",
        email: "a@b.test",
    };
    engine.connect(&account, &password, Some(&second)).unwrap();
    engine.item(&uuid(ID)).unwrap();
    let revision = RevisionDate::parse("2026-11-16T10:00:00.000Z").unwrap();
    engine
        .update_item(&uuid(ID), &revision, &json!({"name": "n"}), Some(&uuid(ORG)))
        .unwrap();
    let calls = handle.calls();
    assert_eq!(
        calls[0],
        Call::Connect {
            server: "https://vault.test".into(),
            email: "a@b.test".into(),
            password: Secret::from("fake-password"),
            code: Some(Secret::from("123456")),
        }
    );
    assert!(!format!("{:?}", calls[0]).contains("fake-password"));
    assert_eq!(calls[1], Call::Item { id: ID.into() });
    assert_eq!(
        calls[2],
        Call::UpdateItem {
            id: ID.into(),
            expected_revision: "2026-11-16T10:00:00.000Z".into(),
            patch: json!({"name": "n"}),
            organization_id: Some(ORG.into()),
        }
    );
}

#[test]
fn killed_engine_is_locked_and_counts_the_kill() {
    let handle = FakeHandle::new();
    let mut engine = handle.engine();
    engine.kill();
    assert!(!engine.alive());
    assert_eq!(code(engine.sync()), Some("engine-locked".into()));
    assert_eq!(handle.kills(), 1);
    handle.set_alive(true);
    assert!(engine.alive());
}

#[test]
fn sharing_calls_carry_every_collection() {
    let handle = FakeHandle::new();
    handle.push_ok().push(Reply::Text("c1".into())).push_ok();
    let mut engine = handle.engine();
    let collections = [uuid(ID), uuid(ORG)];
    engine.check_sharing(&uuid(ORG), &collections).unwrap();
    assert_eq!(engine.create_collection(&uuid(ORG), "Equipe").unwrap(), "c1");
    engine.share_item(&uuid(ID), &uuid(ORG), &collections).unwrap();
    assert_eq!(
        handle.calls()[2],
        Call::ShareItem {
            id: ID.into(),
            organization_id: ORG.into(),
            collection_ids: vec![ID.into(), ORG.into()],
        }
    );
}

#[test]
fn factory_serves_engines_in_order_and_logs_starts() {
    let factory = FakeFactory::new();
    let first = factory.engine();
    factory.push_failure("engine-timeout");
    assert!(factory.start(Path::new("d"), "p1").is_ok());
    assert_eq!(code(factory.start(Path::new("d"), "p2")), Some("engine-timeout".into()));
    assert_eq!(code(factory.start(Path::new("d"), "p3")), Some("engine-init".into()));
    assert_eq!(first.kills(), 0);
    assert_eq!(factory.starts()[0], (PathBuf::from("d"), "p1".to_owned()));
    assert_eq!(factory.starts().len(), 3);
}
