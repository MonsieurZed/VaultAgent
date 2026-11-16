//! Routes de lecture : `catalog`, `write-preview`, `sharing`, `lock` et `request-unlock`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::service::{Client, ITEM_ID, NEXT_REVISION, REVISION, Service, VaultState, demo_item, item_reply};
use serde_json::json;
use vaultagent_engine::{Collection, Folder, Organization, Reply};

/// Aide de test : service.
fn service() -> Service {
    Service::new(VaultState::Unlocked, Client::Paired)
}

#[test]
fn catalog_syncs_first_and_carries_presences_but_no_value() {
    let service = service();
    let mut note = demo_item(REVISION, "Note");
    note.kind = 2;
    service.script(vec![
        Reply::Unit,
        Reply::Items(vec![demo_item(REVISION, "Compte"), note]),
    ]);
    let (status, body) = service.call("catalog", &json!({"search": "a".repeat(300)}));
    assert_eq!(status, 200);
    assert_eq!(body.as_array().map(Vec::len), Some(1));
    assert_eq!(body[0]["origins"], json!(["https://example.test"]));
    let text = body.to_string();
    assert!(!text.contains("Fictif-42!") && !text.contains("FAKESEED") && !text.contains("compte@example"));
    let names = service.engine.call_names();
    assert_eq!(names[names.len() - 2..], ["sync", "items"]);
    let search = service.engine.calls().into_iter().find_map(|call| match call {
        vaultagent_engine::Call::Items { search } => search,
        _ => None,
    });
    assert_eq!(search.map(|text| text.chars().count()), Some(200));
}

#[test]
fn locked_vault_refuses_reads_with_423_and_never_calls_the_engine() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    for route in ["catalog", "write-preview", "sharing", "seal"] {
        let body = json!({"itemId": ITEM_ID, "field": "password", "origin": "https://example.test", "publicKey": "x"});
        assert_eq!(service.call(route, &body), (423, json!({"error": "locked"})), "{route}");
    }
    assert!(
        service
            .engine
            .call_names()
            .iter()
            .all(|name| *name == "connect" || *name == "lock")
    );
}

#[test]
fn write_preview_lists_folders_items_and_trash_without_archived_entries() {
    let service = service();
    let mut archived = demo_item(REVISION, "Archivée");
    archived.archived_date = Some(REVISION.to_owned());
    let mut trashed = demo_item(NEXT_REVISION, "Corbeille");
    trashed.deleted_date = Some(NEXT_REVISION.to_owned());
    service.script(vec![
        Reply::Unit,
        Reply::Folders(vec![Folder {
            id: "f".into(),
            name: "Dossier".into(),
        }]),
        Reply::Items(vec![demo_item(REVISION, "Actif"), archived.clone()]),
        Reply::Items(vec![trashed, archived]),
    ]);
    let (status, body) = service.call("write-preview", &json!({}));
    assert_eq!(status, 200);
    assert_eq!(body["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["trash"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["trash"][0]["name"], "Corbeille");
    assert_eq!(body["folders"], json!([{"id": "f", "name": "Dossier"}]));
}

#[test]
fn write_preview_detail_gives_presences_and_validates_the_id() {
    let service = service();
    service.script(vec![Reply::Unit, Reply::Unit, Reply::Unit]);
    for bad in [json!("pas-un-uuid"), json!(null), json!(7)] {
        assert_eq!(
            service.call("write-preview", &json!({"itemId": bad})),
            (400, json!({"error": "invalid-item"}))
        );
    }
    let mut archived = demo_item(REVISION, "Archivée");
    archived.archived_date = Some(REVISION.to_owned());
    service.script(vec![
        Reply::Unit,
        item_reply(demo_item(REVISION, "Compte")),
        Reply::Unit,
        item_reply(archived),
    ]);
    let (_, body) = service.call("write-preview", &json!({"itemId": ITEM_ID}));
    assert_eq!(
        body["has"],
        json!({"username": true, "password": true, "totp": true, "notes": false})
    );
    assert!(!body.to_string().contains("Fictif-42!"));
    assert_eq!(
        service.call("write-preview", &json!({"itemId": ITEM_ID})),
        (400, json!({"error": "item-refused"}))
    );
}

#[test]
fn sharing_lists_organizations_with_collection_rights() {
    let service = service();
    service.script(vec![
        Reply::Unit,
        Reply::Organizations(vec![Organization {
            id: "o".into(),
            name: "Org".into(),
            role: 0,
            collections: vec![Collection {
                id: "c".into(),
                name: "Col".into(),
                can_edit: false,
            }],
        }]),
    ]);
    let (_, body) = service.call("sharing", &json!({}));
    assert_eq!(body["organizations"][0]["collections"][0]["canEdit"], false);
}

#[test]
fn engine_failures_are_relayed_with_their_stage_code() {
    let service = service();
    service.script(vec![Reply::Failure(vaultagent_engine::stage_error("sync", "network"))]);
    assert_eq!(
        service.call("catalog", &json!({})),
        (400, json!({"error": "sync:network"}))
    );
}

#[test]
fn lock_kills_the_engine_and_status_then_reports_locked() {
    let service = service();
    assert_eq!(service.call("lock", &json!({})), (200, json!({"locked": true})));
    assert_eq!(service.engine.kills(), 1);
    let (_, status) = service.call("status", &json!({}));
    assert_eq!(
        (status["unlocked"].clone(), status["expiresUtc"].clone()),
        (json!(false), json!(null))
    );
}

#[test]
fn request_unlock_is_deduplicated_and_unlocked_vaults_answer_directly() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let (_, first) = service.call("request-unlock", &json!({}));
    let (_, second) = service.call("request-unlock", &json!({}));
    assert_eq!(
        (first["state"].clone(), first["requestId"].clone()),
        (json!("pending"), second["requestId"].clone())
    );
    let (_, status) = service.call("status", &json!({}));
    assert_eq!(status["unlockRequest"]["requestId"], first["requestId"]);
    assert_eq!(status["approvalRequests"], json!([]));
    let open = Service::new(VaultState::Unlocked, Client::Paired);
    assert_eq!(
        open.call("request-unlock", &json!({})).1,
        json!({"state": "unlocked", "unlocked": true})
    );
}
