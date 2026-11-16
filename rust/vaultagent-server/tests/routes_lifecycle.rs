//! Cycle de vie de `write` : corbeille, restauration, renommage et suppression de dossier.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::service::{
    Client, FOLDER_ID, ITEM_ID, NEXT_REVISION, REVISION, Service, VaultState, demo_item, item_reply,
};
use serde_json::{Value, json};
use vaultagent_engine::{Folder, Item, Reply};

const OP: &str = "00000000-0000-4000-8000-0000000000b1";
const ORG: &str = "00000000-0000-4000-8000-000000000009";
const OTHER: &str = "00000000-0000-4000-8000-0000000000f2";

/// Aide de test : service.
fn service() -> Service {
    Service::new(VaultState::Unlocked, Client::Paired)
}

/// Aide de test : trashed.
fn trashed(revision: &str) -> Item {
    Item {
        deleted_date: Some(revision.to_owned()),
        ..demo_item(revision, "Compte")
    }
}

/// Aide de test : delete body.
fn delete_body() -> Value {
    json!({"operationId": OP, "action": "delete", "itemId": ITEM_ID, "expectedRevisionDate": REVISION})
}

/// Aide de test : folder.
fn folder(id: &str, name: &str) -> Folder {
    Folder {
        id: id.to_owned(),
        name: name.to_owned(),
    }
}

#[test]
fn delete_verifies_by_readback_and_replays_the_receipt() {
    let service = service();
    service.script(vec![
        Reply::Unit,
        item_reply(demo_item(REVISION, "Compte")),
        Reply::Unit,
        item_reply(trashed(NEXT_REVISION)),
    ]);
    let (status, receipt) = service.call("write", &delete_body());
    assert_eq!((status, receipt["revisionDate"].clone()), (200, json!(NEXT_REVISION)));
    assert_eq!(service.engine.call_names()[1..], ["sync", "item", "trash_item", "item"]);
    let replay = service.call("write", &delete_body());
    assert_eq!(replay.1["replayed"], true);
    assert_eq!(service.engine.call_names().len(), 5);
}

#[test]
fn restore_needs_a_trashed_item_and_the_revision_that_was_read() {
    let service = service();
    let restore = json!({"operationId": OP, "action": "restore", "itemId": ITEM_ID, "expectedRevisionDate": REVISION});
    service.script(vec![Reply::Unit, item_reply(demo_item(REVISION, "Compte"))]);
    assert_eq!(service.call("write", &restore), (400, json!({"error": "item-refused"})));
    let other = json!({"operationId": "00000000-0000-4000-8000-0000000000b2", "action": "restore", "itemId": ITEM_ID,
        "expectedRevisionDate": "2026-01-01T00:00:00.000Z"});
    service.script(vec![Reply::Unit, item_reply(trashed(REVISION))]);
    assert_eq!(
        service.call("write", &other),
        (400, json!({"error": "revision-conflict"}))
    );
    let ok = json!({"operationId": "00000000-0000-4000-8000-0000000000b3", "action": "restore", "itemId": ITEM_ID,
        "expectedRevisionDate": REVISION});
    service.script(vec![
        Reply::Unit,
        item_reply(trashed(REVISION)),
        Reply::Unit,
        item_reply(demo_item(NEXT_REVISION, "Compte")),
    ]);
    let (status, receipt) = service.call("write", &ok);
    assert_eq!((status, receipt["action"].clone()), (200, json!("restore")));
}

#[test]
fn delete_refuses_a_trashed_item_and_a_wrong_owner() {
    let service = service();
    service.script(vec![Reply::Unit, item_reply(trashed(REVISION))]);
    assert_eq!(
        service.call("write", &delete_body()),
        (400, json!({"error": "item-refused"}))
    );
    let mut owned = delete_body();
    owned["organizationId"] = json!(ORG);
    service.script(vec![Reply::Unit, item_reply(demo_item(REVISION, "Compte"))]);
    assert_eq!(service.call("write", &owned), (400, json!({"error": "item-refused"})));
}

#[test]
fn a_shared_item_needs_the_collection_right_before_anything_is_written() {
    let service = service();
    let shared = Item {
        organization_id: Some(ORG.to_owned()),
        collection_ids: vec![OTHER.to_owned()],
        ..demo_item(REVISION, "Commune")
    };
    let mut body = delete_body();
    body["organizationId"] = json!(ORG);
    service.script(vec![
        Reply::Unit,
        item_reply(shared),
        Reply::Failure(vaultagent_engine::stage_error("sharing", "refused")),
    ]);
    assert_eq!(service.call("write", &body), (400, json!({"error": "sharing:refused"})));
    assert!(service.engine.call_names().contains(&"check_sharing"));
    assert!(!service.engine.call_names().contains(&"trash_item"));
}

#[test]
fn rename_folder_checks_the_name_and_the_folder() {
    let service = service();
    let rename = |name: &str| json!({"operationId": OP, "action": "update-folder", "folderId": FOLDER_ID, "item": {"name": name}});
    let all = vec![folder(FOLDER_ID, "Ancien"), folder(OTHER, "Pris")];
    service.script(vec![Reply::Unit, Reply::Folders(all.clone())]);
    assert_eq!(
        service.call("write", &rename("Pris")),
        (400, json!({"error": "duplicate-folder"}))
    );
    service.script(vec![Reply::Unit, Reply::Folders(vec![folder(OTHER, "Pris")])]);
    assert_eq!(
        service.call("write", &rename("Neuf")),
        (400, json!({"error": "folder-missing"}))
    );
    service.script(vec![
        Reply::Unit,
        Reply::Folders(all),
        Reply::Unit,
        Reply::Folders(vec![folder(FOLDER_ID, "Neuf")]),
    ]);
    let (status, receipt) = service.call("write", &rename("Neuf"));
    assert_eq!((status, receipt["id"].clone()), (200, json!(FOLDER_ID)));
}

#[test]
fn delete_folder_is_confirmed_when_the_folder_is_gone() {
    let service = service();
    let body = json!({"operationId": OP, "action": "delete-folder", "folderId": FOLDER_ID});
    service.script(vec![
        Reply::Unit,
        Reply::Folders(vec![folder(FOLDER_ID, "A")]),
        Reply::Unit,
        Reply::Folders(Vec::new()),
    ]);
    assert_eq!(service.call("write", &body).0, 200);
    assert_eq!(service.engine.call_names().last(), Some(&"folders"));
}

#[test]
fn an_effect_never_seen_gives_write_indeterminate_after_five_waits() {
    let service = service();
    let mut replies = vec![Reply::Unit, item_reply(demo_item(REVISION, "Compte")), Reply::Unit];
    replies.extend((0..5).map(|_| item_reply(demo_item(REVISION, "Compte"))));
    service.script(replies);
    assert_eq!(
        service.call("write", &delete_body()),
        (400, json!({"error": "write-indeterminate"}))
    );
    assert_eq!(service.pause.waits(), vec![200; 5]);
    assert_eq!(
        service.call("write", &delete_body()),
        (400, json!({"error": "write-indeterminate"}))
    );
}

#[test]
fn malformed_forms_are_refused_before_the_engine() {
    let service = service();
    for bad in [
        json!({"operationId": OP, "action": "delete", "itemId": ITEM_ID, "expectedRevisionDate": "nope"}),
        json!({"operationId": OP, "action": "delete", "itemId": ITEM_ID, "expectedRevisionDate": REVISION, "folderId": FOLDER_ID}),
        json!({"operationId": OP, "action": "delete-folder", "folderId": FOLDER_ID, "itemId": ITEM_ID}),
        json!({"operationId": OP, "action": "update-folder", "folderId": FOLDER_ID, "item": {"name": ""}}),
        json!({"operationId": OP, "action": "restore", "itemId": ITEM_ID, "expectedRevisionDate": REVISION, "patch": {}}),
    ] {
        assert_eq!(
            service.call("write", &bad),
            (400, json!({"error": "invalid-write"})),
            "{bad}"
        );
    }
    assert_eq!(service.engine.call_names(), vec!["connect"]);
}
