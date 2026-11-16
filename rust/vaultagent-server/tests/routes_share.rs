//! Route `share` : aperçu, transfert confirmé, création de collection, idempotence et refus.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::service::{Client, ITEM_ID, REVISION, Service, VaultState, demo_item, item_reply};
use serde_json::{Value, json};
use vaultagent_engine::{Collection, Item, Organization, Reply};

const OP: &str = "00000000-0000-4000-8000-0000000000b1";
const ORG: &str = "00000000-0000-4000-8000-000000000009";
const COL: &str = "00000000-0000-4000-8000-0000000000e1";
const NEW_COL: &str = "00000000-0000-4000-8000-0000000000e2";

/// Aide de test : service.
fn service() -> Service {
    Service::new(VaultState::Unlocked, Client::Paired)
}

/// Aide de test : organization.
fn organization(collections: &[(&str, &str)]) -> Organization {
    Organization {
        id: ORG.to_owned(),
        name: "Org".to_owned(),
        role: 0,
        collections: collections
            .iter()
            .map(|(id, name)| Collection {
                id: (*id).to_owned(),
                name: (*name).to_owned(),
                can_edit: true,
            })
            .collect(),
    }
}

/// Aide de test : apply body.
fn apply_body() -> Value {
    json!({"action": "apply", "operationId": OP, "organizationId": ORG, "itemId": ITEM_ID,
        "collectionIds": [COL], "expectedRevisionDate": REVISION, "confirmOwnershipTransfer": true})
}

/// Aide de test : moved.
fn moved() -> Item {
    Item {
        organization_id: Some(ORG.to_owned()),
        collection_ids: vec![COL.to_owned()],
        ..demo_item(REVISION, "Compte")
    }
}

/// Aide de test : head.
fn head(replies: Vec<Reply>) -> Vec<Reply> {
    let mut all = vec![
        Reply::Unit,
        Reply::Organizations(vec![organization(&[(COL, "Existante")])]),
        Reply::Unit,
    ];
    all.extend(replies);
    all
}

#[test]
fn preview_describes_the_transfer_and_writes_nothing() {
    let service = service();
    service.script(head(vec![item_reply(demo_item(REVISION, "Compte"))]));
    let body = json!({"action": "preview", "organizationId": ORG, "itemId": ITEM_ID, "collectionIds": [COL]});
    let (status, preview) = service.call("share", &body);
    assert_eq!(status, 200);
    assert_eq!(preview["organizationName"], "Org");
    assert_eq!(
        (
            preview["requiresConfirmation"].clone(),
            preview["transfersOwnership"].clone()
        ),
        (json!(true), json!(true))
    );
    assert_eq!(preview["expectedRevisionDate"], REVISION);
    assert!(!service.engine.call_names().contains(&"share_item"));
    assert!(!service.dir.path().join("data/share-receipts.json").exists());
}

#[test]
fn apply_needs_the_confirmation_and_the_read_revision() {
    let service = service();
    let mut unconfirmed = apply_body();
    unconfirmed["confirmOwnershipTransfer"] = json!(false);
    service.script(head(vec![item_reply(demo_item(REVISION, "Compte"))]));
    assert_eq!(
        service.call("share", &unconfirmed),
        (400, json!({"error": "revision-conflict"}))
    );
    let mut stale = apply_body();
    stale["expectedRevisionDate"] = json!("2026-01-01T00:00:00.000Z");
    service.script(head(vec![item_reply(demo_item(REVISION, "Compte"))]));
    assert_eq!(
        service.call("share", &stale),
        (400, json!({"error": "revision-conflict"}))
    );
    assert!(!service.engine.call_names().contains(&"share_item"));
}

#[test]
fn apply_transfers_once_and_replays_the_same_receipt() {
    let service = service();
    service.script(head(vec![
        item_reply(demo_item(REVISION, "Compte")),
        Reply::Unit,
        Reply::Unit,
        item_reply(moved()),
    ]));
    let (status, receipt) = service.call("share", &apply_body());
    assert_eq!(status, 200);
    assert_eq!(
        (receipt["organizationId"].clone(), receipt["verified"].clone()),
        (json!(ORG), json!(true))
    );
    assert_eq!(receipt["id"], ITEM_ID);
    let calls = service.engine.call_names().len();
    let replay = service.call("share", &apply_body());
    assert_eq!(replay.1["replayed"], true);
    assert_eq!(service.engine.call_names().len(), calls);
    let mut other = apply_body();
    other["collectionIds"] = json!([NEW_COL]);
    assert_eq!(
        service.call("share", &other),
        (400, json!({"error": "operation-conflict"}))
    );
}

#[test]
fn an_unverified_transfer_is_indeterminate_and_never_repeated() {
    let service = service();
    let mut replies = head(vec![item_reply(demo_item(REVISION, "Compte")), Reply::Unit]);
    for _ in 0..5 {
        replies.push(Reply::Unit);
        replies.push(item_reply(demo_item(REVISION, "Compte")));
    }
    service.script(replies);
    assert_eq!(
        service.call("share", &apply_body()),
        (400, json!({"error": "share-indeterminate"}))
    );
    assert_eq!(service.pause.waits(), vec![200; 5]);
    assert_eq!(
        service.call("share", &apply_body()),
        (400, json!({"error": "share-indeterminate"}))
    );
}

#[test]
fn create_collection_trims_the_name_and_checks_duplicates() {
    let service = service();
    let create =
        |name: &str| json!({"action": "create-collection", "operationId": OP, "organizationId": ORG, "name": name});
    service.script(vec![
        Reply::Unit,
        Reply::Organizations(vec![organization(&[(COL, "Existante")])]),
    ]);
    assert_eq!(
        service.call("share", &create(" Existante ")),
        (400, json!({"error": "duplicate-collection"}))
    );
    service.script(vec![
        Reply::Unit,
        Reply::Organizations(vec![organization(&[(COL, "Existante")])]),
        Reply::Text(NEW_COL.to_owned()),
        Reply::Unit,
        Reply::Organizations(vec![organization(&[(COL, "Existante"), (NEW_COL, "Nouvelle")])]),
    ]);
    let (status, receipt) = service.call("share", &create("  Nouvelle "));
    assert_eq!((status, receipt["id"].clone()), (200, json!(NEW_COL)));
    assert_eq!(receipt["action"], "create-collection");
}

#[test]
fn unknown_organization_attachments_and_malformed_bodies_are_refused() {
    let service = service();
    service.script(vec![Reply::Unit, Reply::Organizations(Vec::new())]);
    assert_eq!(
        service.call("share", &apply_body()),
        (400, json!({"error": "organization-refused"}))
    );
    let attached = Item {
        has_attachments: true,
        ..demo_item(REVISION, "Avec pièce jointe")
    };
    service.script(head(vec![item_reply(attached)]));
    let body = json!({"action": "preview", "organizationId": ORG, "itemId": ITEM_ID, "collectionIds": [COL]});
    assert_eq!(service.call("share", &body), (400, json!({"error": "item-refused"})));
    let mut unknown = apply_body();
    unknown["surprise"] = json!(true);
    assert_eq!(
        service.call("share", &unknown),
        (400, json!({"error": "invalid-share"}))
    );
    let mut without_operation = apply_body();
    without_operation.as_object_mut().unwrap().remove("operationId");
    assert_eq!(
        service.call("share", &without_operation),
        (400, json!({"error": "invalid-share"}))
    );
}
