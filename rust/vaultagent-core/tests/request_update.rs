//! Validation des requêtes d'écriture : mise à jour, patch et cycle de vie des fiches et des dossiers.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::{ErrorCode, WriteAction, WriteRequest};

const OP: &str = "00000000-0000-4000-8000-000000000002";
const ITEM: &str = "00000000-0000-4000-8000-000000000001";
const FOLDER: &str = "00000000-0000-4000-8000-000000000007";
const ORG: &str = "00000000-0000-4000-8000-000000000008";
const REV: &str = "2026-11-08T00:00:00.000Z";

/// Requête `update` valide de référence.
fn update(patch: Value) -> Value {
    json!({"operationId": OP, "action": "update", "itemId": ITEM, "expectedRevisionDate": REV, "patch": patch})
}

/// Requête de corbeille de référence.
fn trash(action: &str) -> Value {
    json!({"operationId": OP, "action": action, "itemId": ITEM, "expectedRevisionDate": REV})
}

/// Vérifie que toutes les requêtes sont refusées avec le code donné.
fn refused(code: &ErrorCode, bodies: Vec<Value>) {
    for body in bodies {
        assert_eq!(WriteRequest::parse(&body).unwrap_err(), *code, "{body}");
    }
}

#[test]
fn update_request_rules() {
    let request = WriteRequest::parse(&update(json!({"name": "Renommée"}))).unwrap();
    assert!(matches!(request.action(), WriteAction::Update { item_id, .. } if item_id.as_str() == ITEM));
    let mut bad_id = update(json!({"name": "x"}));
    bad_id["itemId"] = json!("nope");
    let mut no_revision = update(json!({"name": "x"}));
    no_revision.as_object_mut().unwrap().remove("expectedRevisionDate");
    let mut bad_revision = update(json!({"name": "x"}));
    bad_revision["expectedRevisionDate"] = json!("hier");
    let mut with_item = update(json!({"name": "x"}));
    with_item["item"] = json!({"name": "x"});
    let mut with_folder = update(json!({"name": "x"}));
    with_folder["folderId"] = json!(FOLDER);
    refused(
        &ErrorCode::InvalidWrite,
        vec![bad_id, no_revision, bad_revision, with_item, with_folder],
    );
}

#[test]
fn update_patch_rules() {
    refused(
        &ErrorCode::InvalidPatch,
        vec![
            update(json!({"fields": []})),
            update(json!({})),
            update(json!(null)),
            update(json!([])),
            update(json!({"name": " "})),
            update(json!({"name": 3})),
            update(json!({"notes": "n".repeat(12_001)})),
            update(json!({"password": "p".repeat(8193)})),
            update(json!({"folderId": "nope"})),
            update(json!({"addUris": ["http://nvr.example.test"]})),
            update(json!({"addUris": []})),
            update(json!({"addUris": "https://nvr.example.test"})),
            update(json!({"removeUris": []})),
        ],
    );
    let mut missing_patch = update(json!(null));
    missing_patch.as_object_mut().unwrap().remove("patch");
    refused(&ErrorCode::InvalidPatch, vec![missing_patch]);
    assert!(
        WriteRequest::parse(&update(
            json!({"folderId": null, "addUris": ["https://nvr.example.test"]})
        ))
        .is_ok()
    );
}

#[test]
fn update_remove_uris_accepts_any_named_string() {
    assert!(WriteRequest::parse(&update(json!({"removeUris": ["http://legacy.example.test"]}))).is_ok());
    let mut with_org = update(json!({"name": "x"}));
    with_org["organizationId"] = json!(ORG);
    assert_eq!(
        WriteRequest::parse(&with_org)
            .unwrap()
            .organization_id()
            .unwrap()
            .as_str(),
        ORG
    );
}

#[test]
fn item_lifecycle_rules() {
    for action in ["delete", "restore"] {
        let request = WriteRequest::parse(&trash(action)).unwrap();
        assert_eq!(request.action().name(), action);
        let mut shared = trash(action);
        shared["organizationId"] = json!(ORG);
        assert!(WriteRequest::parse(&shared).is_ok());
        let mut with_patch = trash(action);
        with_patch["patch"] = json!({"name": "X"});
        let mut with_item = trash(action);
        with_item["item"] = json!({"name": "X"});
        let mut with_folder = trash(action);
        with_folder["folderId"] = json!(FOLDER);
        let mut bad_id = trash(action);
        bad_id["itemId"] = json!("pas-un-uuid");
        let mut no_revision = trash(action);
        no_revision.as_object_mut().unwrap().remove("expectedRevisionDate");
        let mut unknown = trash(action);
        unknown["permanent"] = json!(true);
        refused(
            &ErrorCode::InvalidWrite,
            vec![with_patch, with_item, with_folder, bad_id, no_revision, unknown],
        );
    }
}

#[test]
fn folder_lifecycle_rules() {
    let rename = json!({"operationId": OP, "action": "update-folder", "folderId": FOLDER, "item": {"name": "Neuf"}});
    assert!(
        matches!(WriteRequest::parse(&rename).unwrap().action(), WriteAction::UpdateFolder { name, .. } if name.as_str() == "Neuf")
    );
    let delete = json!({"operationId": OP, "action": "delete-folder", "folderId": FOLDER});
    assert!(matches!(
        WriteRequest::parse(&delete).unwrap().action(),
        WriteAction::DeleteFolder { .. }
    ));
    let with = |base: &Value, key: &str, value: Value| {
        let mut body = base.clone();
        body[key] = value;
        body
    };
    refused(
        &ErrorCode::InvalidWrite,
        vec![
            json!({"operationId": OP, "action": "delete-folder"}),
            with(&delete, "itemId", json!(ITEM)),
            with(&delete, "expectedRevisionDate", json!(REV)),
            with(&delete, "organizationId", json!(ORG)),
            with(&delete, "item", json!({"name": "X"})),
            with(&delete, "patch", json!({"name": "X"})),
            with(&delete, "folderId", json!("nope")),
            json!({"operationId": OP, "action": "update-folder", "folderId": FOLDER}),
            with(&rename, "item", json!({"name": "X", "extra": 1})),
            with(&rename, "item", json!({"name": "  "})),
            with(&rename, "item", json!({"name": "a".repeat(251)})),
            with(&rename, "itemId", json!(ITEM)),
        ],
    );
}
