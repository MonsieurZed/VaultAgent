//! Validation des requêtes d'écriture : chaque refus de `validate` et `validateLifecycle` de Node.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::{ErrorCode, ItemKind, WriteAction, WriteRequest};

const OP: &str = "00000000-0000-4000-8000-000000000002";
const ITEM: &str = "00000000-0000-4000-8000-000000000001";
const FOLDER: &str = "00000000-0000-4000-8000-000000000007";
const ORG: &str = "00000000-0000-4000-8000-000000000008";
const REV: &str = "2026-11-08T00:00:00.000Z";

/// Requête `create` valide de référence.
fn create() -> Value {
    json!({"operationId": OP, "action": "create", "item": {"type": 1, "name": "Exemple factice",
        "login": {"username": "u@example.invalid", "password": "Fictif-1", "totp": "FAKE", "uris": [{"uri": "https://example.test"}]}}})
}

/// Vérifie que toutes les requêtes sont refusées avec le code donné.
fn refused(code: &ErrorCode, bodies: Vec<Value>) {
    for body in bodies {
        assert_eq!(WriteRequest::parse(&body).unwrap_err(), *code, "{body}");
    }
}

/// Copie de la requête de création dont `item` est modifié par la fonction donnée.
fn create_with(change: impl Fn(&mut Value)) -> Value {
    let mut body = create();
    change(&mut body["item"]);
    body
}

#[test]
fn valid_create_keeps_fields() {
    let request = WriteRequest::parse(&create()).unwrap();
    let WriteAction::Create { item } = request.action() else {
        panic!("create attendu")
    };
    assert_eq!(item.kind, ItemKind::Login);
    assert_eq!(item.name.as_str(), "Exemple factice");
    assert_eq!(item.login.as_ref().unwrap().uris[0].as_str(), "https://example.test");
    assert_eq!(request.raw(), &create());
    assert!(!format!("{request:?}").contains("Fictif-1"));
}

#[test]
fn top_level_refusals() {
    let mut extra = create();
    extra["get"] = json!("password");
    refused(
        &ErrorCode::InvalidWrite,
        vec![
            extra,
            json!(null),
            json!([]),
            json!("x"),
            json!({"action": "create"}),
            json!({"operationId": "pas-un-uuid", "action": "create", "item": {"type": 2, "name": "x"}}),
            json!({"operationId": OP, "action": "purge", "item": {"name": "x"}}),
            json!({"operationId": OP, "action": 3}),
            json!({"operationId": OP, "action": "create-folder", "item": {"name": "x"}, "organizationId": "nope"}),
            json!({"operationId": OP, "action": "create-folder", "item": {"name": "x"}, "organizationId": ""}),
        ],
    );
}

#[test]
fn create_refusals() {
    refused(
        &ErrorCode::InvalidWrite,
        vec![
            create_with(|i| {
                i["type"] = json!(3);
            }),
            create_with(|i| {
                i["type"] = json!("1");
            }),
            create_with(|i| {
                i["name"] = json!("  ");
            }),
            create_with(|i| {
                i["name"] = json!("a".repeat(501));
            }),
            create_with(|i| {
                i["name"] = json!("a\nb");
            }),
            create_with(|i| {
                i["name"] = json!(5);
            }),
            create_with(|i| {
                i["folderId"] = json!("nope");
            }),
            create_with(|i| {
                i["extra"] = json!(1);
            }),
            create_with(|i| {
                i["notes"] = json!("n".repeat(12_001));
            }),
            create_with(|i| {
                i["notes"] = json!(null);
            }),
            create_with(|i| {
                i["login"]["password"] = json!("p".repeat(8193));
            }),
            create_with(|i| {
                i["login"]["extra"] = json!(1);
            }),
            create_with(|i| {
                i["login"] = json!(null);
            }),
            create_with(|i| {
                i.as_object_mut().unwrap().remove("login");
            }),
            create_with(|i| {
                i["login"]["uris"] = json!([{"uri": "http://example.test"}]);
            }),
            create_with(|i| {
                i["login"]["uris"] = json!([{"uri": "https://u:p@example.test"}]);
            }),
            create_with(|i| {
                i["login"]["uris"] = json!([{"uri": "https://example.test/?q=1"}]);
            }),
            create_with(|i| {
                i["login"]["uris"] = json!([{"uri": "https://example.test/#f"}]);
            }),
            create_with(|i| {
                i["login"]["uris"] = json!([{"uri": "https://example.test", "extra": 1}]);
            }),
            create_with(|i| {
                i["login"]["uris"] = json!(["https://example.test"]);
            }),
            create_with(|i| {
                i["login"]["uris"] = json!(
                    (0..21)
                        .map(|n| json!({"uri": format!("https://h{n}.example.test")}))
                        .collect::<Vec<_>>()
                );
            }),
            create_with(|i| {
                i["login"]["uris"] = json!("https://example.test");
            }),
        ],
    );
    let mut with_item_id = create();
    with_item_id["itemId"] = json!(ITEM);
    let mut with_folder = create();
    with_folder["folderId"] = json!(FOLDER);
    let mut with_patch = create();
    with_patch["patch"] = json!({"name": "x"});
    let mut with_revision = create();
    with_revision["expectedRevisionDate"] = json!(REV);
    refused(
        &ErrorCode::InvalidWrite,
        vec![with_item_id, with_folder, with_patch, with_revision],
    );
}

#[test]
fn secure_note_and_collections() {
    let note = json!({"operationId": OP, "action": "create", "item": {"type": 2, "name": "Note", "notes": "texte"}});
    assert!(
        matches!(WriteRequest::parse(&note).unwrap().action(), WriteAction::Create { item } if item.kind == ItemKind::SecureNote)
    );
    let mut with_login = note.clone();
    with_login["item"]["login"] = json!({});
    let shared = |ids: Value| json!({"operationId": OP, "action": "create", "item": {"type": 2, "name": "N", "organizationId": ORG, "collectionIds": ids}});
    let too_many: Vec<Value> = (0..33).map(|_| json!(FOLDER)).collect();
    let loose =
        json!({"operationId": OP, "action": "create", "item": {"type": 2, "name": "N", "collectionIds": [FOLDER]}});
    refused(
        &ErrorCode::InvalidWrite,
        vec![
            with_login,
            shared(json!([])),
            shared(json!(too_many)),
            shared(json!(["nope"])),
            shared(json!(null)),
            loose,
        ],
    );
    assert!(WriteRequest::parse(&shared(json!([FOLDER]))).is_ok());
    assert!(
        WriteRequest::parse(
            &json!({"operationId": OP, "action": "create", "item": {"type": 2, "name": "N", "collectionIds": []}})
        )
        .is_ok()
    );
}

#[test]
fn create_folder_rules() {
    let folder = |item: Value| json!({"operationId": OP, "action": "create-folder", "item": item});
    assert!(WriteRequest::parse(&folder(json!({"name": "Exemples"}))).is_ok());
    refused(
        &ErrorCode::InvalidWrite,
        vec![
            folder(json!({"name": " "})),
            folder(json!({"name": "a".repeat(251)})),
            folder(json!({"name": "a\u{1}"})),
            folder(json!({"name": "x", "extra": 1})),
            folder(json!({})),
            folder(json!(null)),
            json!({"operationId": OP, "action": "create-folder"}),
            json!({"operationId": OP, "action": "create-folder", "item": {"name": "x"}, "itemId": ITEM}),
            json!({"operationId": OP, "action": "create-folder", "item": {"name": "x"}, "folderId": FOLDER}),
            json!({"operationId": OP, "action": "create-folder", "item": {"name": "x"}, "patch": {}}),
            json!({"operationId": OP, "action": "create-folder", "item": {"name": "x"}, "expectedRevisionDate": REV}),
        ],
    );
}
