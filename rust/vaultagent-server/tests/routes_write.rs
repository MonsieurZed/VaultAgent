//! Route `write` : création, mise à jour, dossiers, idempotence, relecture et refus avant écriture.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::service::{
    Client, FOLDER_ID, ITEM_ID, NEXT_REVISION, REVISION, Service, VaultState, demo_item, item_reply,
};
use serde_json::{Value, json};
use vaultagent_engine::{Folder, PasswordHistoryEntry, Reply, Secret, stage_error};

const OP: &str = "00000000-0000-4000-8000-0000000000b1";
const NEW_ID: &str = "00000000-0000-4000-8000-0000000000d1";

/// Aide de test : service.
fn service() -> Service {
    Service::new(VaultState::Unlocked, Client::Paired)
}

/// Aide de test : create body.
fn create_body() -> Value {
    json!({"operationId": OP, "action": "create", "item": {"type": 1, "name": "Neuf", "login": {"password": "Nouveau-1!"}}})
}

/// Aide de test : created.
fn created() -> vaultagent_engine::Item {
    let mut item = demo_item(NEXT_REVISION, "Neuf");
    NEW_ID.clone_into(&mut item.id);
    item.login.as_mut().unwrap().username = None;
    item.login.as_mut().unwrap().totp = None;
    item.login.as_mut().unwrap().uris.clear();
    item.login.as_mut().unwrap().password = Some(Secret::from("Nouveau-1!"));
    item
}

/// Aide de test : create script.
fn create_script() -> Vec<Reply> {
    vec![
        Reply::Unit,
        Reply::Items(Vec::new()),
        Reply::Text(NEW_ID.to_owned()),
        item_reply(created()),
    ]
}

#[test]
fn create_returns_a_verified_receipt_and_the_ledger_holds_no_secret() {
    let service = service();
    service.script(create_script());
    let (status, receipt) = service.call("write", &create_body());
    assert_eq!(status, 200);
    assert_eq!(receipt["revisionDate"], NEXT_REVISION);
    assert_eq!(
        service.engine.call_names()[1..],
        ["sync", "items", "create_item", "item"]
    );
    let ledger = std::fs::read_to_string(service.dir.path().join("data/write-receipts.json")).unwrap();
    assert!(ledger.contains(OP) && !ledger.contains("Nouveau-1!"));
    let replay = service.call("write", &create_body());
    assert_eq!(replay.1["replayed"], true);
    assert_eq!(service.engine.call_names().len(), 5);
}

#[test]
fn same_operation_with_another_body_is_a_conflict() {
    let service = service();
    service.script(create_script());
    service.call("write", &create_body());
    let mut other = create_body();
    other["item"]["name"] = json!("Autre");
    assert_eq!(
        service.call("write", &other),
        (400, json!({"error": "operation-conflict"}))
    );
}

#[test]
fn update_checks_the_password_history_and_returns_the_new_revision() {
    let service = service();
    let mut saved = demo_item(NEXT_REVISION, "Compte");
    saved.login.as_mut().unwrap().password = Some(Secret::from("Nouveau-43!"));
    saved.password_history = vec![PasswordHistoryEntry {
        password: Secret::from("Fictif-42!"),
        last_used_date: None,
    }];
    service.script(vec![
        Reply::Unit,
        item_reply(demo_item(REVISION, "Compte")),
        Reply::Unit,
        item_reply(saved),
    ]);
    let body = json!({"operationId": OP, "action": "update", "itemId": ITEM_ID, "expectedRevisionDate": REVISION,
        "patch": {"password": "Nouveau-43!"}});
    let (status, receipt) = service.call("write", &body);
    assert_eq!((status, receipt["revisionDate"].clone()), (200, json!(NEXT_REVISION)));
    assert!(!receipt.to_string().contains("Nouveau-43!"));
}

#[test]
fn update_with_a_stale_revision_never_reaches_the_write() {
    let service = service();
    service.script(vec![Reply::Unit, item_reply(demo_item(NEXT_REVISION, "Compte"))]);
    let body = json!({"operationId": OP, "action": "update", "itemId": ITEM_ID, "expectedRevisionDate": REVISION,
        "patch": {"name": "X"}});
    assert_eq!(
        service.call("write", &body),
        (400, json!({"error": "revision-conflict"}))
    );
    assert!(!service.engine.call_names().contains(&"update_item"));
}

#[test]
fn create_folder_reuses_an_existing_folder_without_creating_another() {
    let service = service();
    let existing = Folder {
        id: FOLDER_ID.to_owned(),
        name: "Dossier".to_owned(),
    };
    service.script(vec![
        Reply::Unit,
        Reply::Folders(vec![existing.clone()]),
        Reply::Folders(vec![existing]),
    ]);
    let body = json!({"operationId": OP, "action": "create-folder", "item": {"name": "Dossier"}});
    let (status, receipt) = service.call("write", &body);
    assert_eq!((status, receipt["id"].clone()), (200, json!(FOLDER_ID)));
    assert!(!service.engine.call_names().contains(&"create_folder"));
}

#[test]
fn a_failure_after_the_intent_forbids_any_blind_repeat() {
    let service = service();
    service.script(vec![
        Reply::Unit,
        Reply::Items(Vec::new()),
        Reply::Failure(stage_error("write", "rejected")),
    ]);
    assert_eq!(
        service.call("write", &create_body()),
        (400, json!({"error": "write-indeterminate"}))
    );
    let calls = service.engine.call_names().len();
    assert_eq!(
        service.call("write", &create_body()),
        (400, json!({"error": "write-indeterminate"}))
    );
    assert_eq!(service.engine.call_names().len(), calls);
}

#[test]
fn readback_retries_five_times_every_200_ms_then_gives_up() {
    let service = service();
    let wrong = || item_reply(demo_item(NEXT_REVISION, "Pas le bon"));
    let mut replies = vec![Reply::Unit, Reply::Items(Vec::new()), Reply::Text(NEW_ID.to_owned())];
    replies.extend((0..5).map(|_| wrong()));
    service.script(replies);
    assert_eq!(
        service.call("write", &create_body()),
        (400, json!({"error": "write-indeterminate"}))
    );
    assert_eq!(service.pause.waits(), vec![200; 5]);
}

#[test]
fn readback_succeeds_on_a_later_attempt() {
    let service = service();
    service.script(vec![
        Reply::Unit,
        Reply::Items(Vec::new()),
        Reply::Text(NEW_ID.to_owned()),
        item_reply(demo_item(NEXT_REVISION, "Pas encore")),
        item_reply(created()),
    ]);
    assert_eq!(service.call("write", &create_body()).0, 200);
    assert_eq!(service.pause.waits(), vec![200]);
}

#[test]
fn checks_before_the_intent_leave_no_trace() {
    let service = service();
    service.script(vec![Reply::Unit, Reply::Items(vec![demo_item(REVISION, "Neuf")])]);
    assert_eq!(
        service.call("write", &create_body()),
        (400, json!({"error": "duplicate-item"}))
    );
    let mut in_folder = create_body();
    in_folder["item"]["folderId"] = json!(FOLDER_ID);
    service.script(vec![Reply::Unit, Reply::Items(Vec::new()), Reply::Folders(Vec::new())]);
    assert_eq!(
        service.call("write", &in_folder),
        (400, json!({"error": "folder-missing"}))
    );
    assert!(!service.dir.path().join("data/write-receipts.json").exists());
    assert!(!service.engine.call_names().contains(&"create_item"));
}

#[test]
fn schemas_are_strict_and_validation_precedes_the_lock_check() {
    let locked = Service::new(VaultState::Locked, Client::Paired);
    let mut unknown = create_body();
    unknown["surprise"] = json!(1);
    assert_eq!(locked.call("write", &unknown), (400, json!({"error": "invalid-write"})));
    assert_eq!(locked.call("write", &create_body()), (423, json!({"error": "locked"})));
    for bad in [
        json!({"operationId": "x", "action": "create", "item": {"type": 1, "name": "N"}}),
        json!({"operationId": OP, "action": "create", "item": {"type": 1, "name": "N", "login": {"uris": [{"uri": "http://a.test"}]}}}),
        json!({"operationId": OP, "action": "teleport"}),
        json!({"operationId": OP, "action": "create-folder", "item": {"name": "  "}}),
    ] {
        assert_eq!(
            locked.call("write", &bad),
            (400, json!({"error": "invalid-write"})),
            "{bad}"
        );
    }
    let bad_patch = json!({"operationId": OP, "action": "update", "itemId": ITEM_ID, "expectedRevisionDate": REVISION,
        "patch": {"addUris": [{"uri": "http://a.test"}]}});
    assert_eq!(locked.call("write", &bad_patch).1["error"], "invalid-patch");
}
