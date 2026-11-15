//! Partage : validation de la requête, contrôles d'avant écriture, aperçu, vérification par relecture, reçu et
//! registre d'idempotence `share-receipts.json` (cahier `sharing.mjs` et `multivault.test.mjs`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::share_checks::{check_confirmation, check_item, check_new_collection, find_organization, preview};
use vaultagent_core::share_verify::{collection_created, item_moved, receipt};
use vaultagent_core::{ErrorCode, ShareAction, ShareRequest, Uuid};

const ORG: &str = "10000000-0000-4000-8000-000000000001";
const COLLECTION: &str = "20000000-0000-4000-8000-000000000001";
const OTHER: &str = "20000000-0000-4000-8000-000000000002";
const ITEM: &str = "00000000-0000-4000-8000-000000000001";
const OPERATION: &str = "00000000-0000-4000-8000-000000000002";

/// Requête `apply` de référence.
fn apply() -> Value {
    json!({"action": "apply", "itemId": ITEM, "organizationId": ORG, "collectionIds": [COLLECTION],
           "operationId": OPERATION, "expectedRevisionDate": "2026-11-08T00:00:00.000Z",
           "confirmOwnershipTransfer": true})
}

/// Valide une requête.
fn parse(raw: Value) -> Result<ShareRequest, ErrorCode> {
    ShareRequest::parse(&raw)
}

/// Fiche personnelle de référence.
fn item() -> Value {
    json!({"id": ITEM, "name": "Perso", "type": 1, "notes": null, "revisionDate": "2026-11-08T00:00:00.000Z",
           "login": {"username": "u", "password": "Fictif-1"}, "fields": [{"name": "a", "value": "b"}],
           "passwordHistory": [{"password": "Fictif-0"}]})
}

/// Organisations de `sharing-info`.
fn organizations() -> Vec<Value> {
    vec![json!({"id": ORG, "name": "Famille factice", "role": 0,
                "collections": [{"id": COLLECTION, "name": "Communs", "canEdit": true}]})]
}

#[test]
fn request_accepts_the_three_actions() {
    let request = parse(apply()).unwrap();
    assert_eq!(request.action(), ShareAction::Apply);
    assert_eq!(request.item_id().unwrap().as_str(), ITEM);
    assert_eq!(request.collection_ids().len(), 1);
    assert!(request.confirmed());
    assert_eq!(request.raw(), &apply());
    let preview =
        parse(json!({"action": "preview", "itemId": ITEM, "organizationId": ORG, "collectionIds": [COLLECTION]}));
    assert!(preview.unwrap().operation_id().is_none());
    let create = parse(
        json!({"action": "create-collection", "organizationId": ORG, "operationId": OPERATION, "name": "  Neuf  "}),
    );
    assert_eq!(create.unwrap().name(), Some("Neuf"));
}

#[test]
fn request_refusals_are_invalid_share() {
    let mut cases: Vec<Value> = vec![json!(null), json!([]), json!({"action": "apply", "vaultId": "x"})];
    let mutate = |edit: &dyn Fn(&mut Value)| {
        let mut base = apply();
        edit(&mut base);
        base
    };
    cases.push(mutate(&|v| v["action"] = json!("delete")));
    cases.push(mutate(&|v| v["organizationId"] = json!("x")));
    cases.push(mutate(&|v| v["itemId"] = json!(1)));
    cases.push(mutate(&|v| v["collectionIds"] = json!([])));
    cases.push(mutate(&|v| v["collectionIds"] = json!(["x"])));
    cases.push(mutate(&|v| v["collectionIds"] = json!(vec![COLLECTION; 33])));
    cases.push(mutate(&|v| v["operationId"] = json!("x")));
    cases.push(mutate(&|v| {
        v.as_object_mut().unwrap().remove("operationId");
    }));
    for name in [
        json!(null),
        json!(""),
        json!("  "),
        json!("a\nb"),
        json!("a".repeat(201)),
    ] {
        cases.push(
            json!({"action": "create-collection", "organizationId": ORG, "operationId": OPERATION, "name": name}),
        );
    }
    for raw in cases {
        assert_eq!(parse(raw.clone()).unwrap_err(), ErrorCode::InvalidShare, "{raw}");
    }
    let edge =
        json!({"action": "preview", "itemId": ITEM, "organizationId": ORG, "collectionIds": vec![COLLECTION; 32]});
    assert!(parse(edge).is_ok());
    let long = json!({"action": "create-collection", "organizationId": ORG, "operationId": OPERATION, "name": "a".repeat(200)});
    assert!(parse(long).is_ok());
}

#[test]
fn organization_and_item_refusals() {
    let request = parse(apply()).unwrap();
    assert!(find_organization(&organizations(), &request).is_ok());
    assert_eq!(
        find_organization(&[], &request).unwrap_err(),
        ErrorCode::OrganizationRefused
    );
    assert!(check_item(&item()).is_ok());
    let refused = [
        json!(null),
        json!({"type": 1, "organizationId": ORG}),
        json!({"type": 1, "deletedDate": "2026-11-01T00:00:00.000Z"}),
        json!({"type": 2, "archivedDate": "2026-11-01T00:00:00.000Z"}),
        json!({"type": 3}),
        json!({"type": 4}),
        json!({"type": 1, "attachments": [{"id": "a"}]}),
    ];
    for fiche in refused {
        assert_eq!(check_item(&fiche).unwrap_err(), ErrorCode::ItemRefused, "{fiche}");
    }
    assert!(check_item(&json!({"type": 2, "attachments": [], "deletedDate": null})).is_ok());
}

#[test]
fn confirmation_needs_flag_and_matching_revision() {
    let fiche = item();
    assert!(check_confirmation(&parse(apply()).unwrap(), &fiche).is_ok());
    let mut raw = apply();
    raw.as_object_mut().unwrap().remove("confirmOwnershipTransfer");
    assert_eq!(
        check_confirmation(&parse(raw).unwrap(), &fiche).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    let mut stale = apply();
    stale["expectedRevisionDate"] = json!("2026-11-09T00:00:00.000Z");
    assert_eq!(
        check_confirmation(&parse(stale).unwrap(), &fiche).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    let mut text = apply();
    text["confirmOwnershipTransfer"] = json!("true");
    assert_eq!(
        check_confirmation(&parse(text).unwrap(), &fiche).unwrap_err(),
        ErrorCode::RevisionConflict
    );
}

#[test]
fn duplicate_collection_uses_the_trimmed_name() {
    let org = organizations().remove(0);
    let request = |name: &str| {
        parse(json!({"action": "create-collection", "organizationId": ORG, "operationId": OPERATION, "name": name}))
            .unwrap()
    };
    assert_eq!(
        check_new_collection(&org, &request(" Communs ")).unwrap_err(),
        ErrorCode::DuplicateCollection
    );
    assert!(check_new_collection(&org, &request("Autre")).is_ok());
}

#[test]
fn preview_has_the_protocol_shape_and_no_secret() {
    let raw = json!({"action": "preview", "itemId": ITEM, "organizationId": ORG, "collectionIds": [COLLECTION]});
    let request = parse(raw).unwrap();
    let shown = serde_json::to_value(preview(&request, &item(), &organizations()[0])).unwrap();
    assert_eq!(
        shown,
        json!({"itemId": ITEM, "name": "Perso", "expectedRevisionDate": "2026-11-08T00:00:00.000Z",
               "organizationId": ORG, "organizationName": "Famille factice", "collectionIds": [COLLECTION],
               "transfersOwnership": true, "requiresConfirmation": true})
    );
    assert!(!shown.to_string().contains("Fictif"));
}

#[test]
fn move_verification_compares_organization_collections_and_fields() {
    let original = item();
    let request = parse(apply()).unwrap();
    let moved = |edit: &dyn Fn(&mut Value)| {
        let mut saved = original.clone();
        saved["organizationId"] = json!(ORG);
        saved["collectionIds"] = json!([COLLECTION]);
        saved["revisionDate"] = json!("2026-11-09T00:00:00.000Z");
        edit(&mut saved);
        saved
    };
    assert!(item_moved(&moved(&|_| {}), &original, &request));
    assert!(!item_moved(
        &moved(&|s| s["organizationId"] = json!(null)),
        &original,
        &request
    ));
    assert!(!item_moved(
        &moved(&|s| s["collectionIds"] = json!([OTHER])),
        &original,
        &request
    ));
    assert!(!item_moved(
        &moved(&|s| s["login"]["password"] = json!("autre")),
        &original,
        &request
    ));
    assert!(!item_moved(
        &moved(&|s| s["passwordHistory"] = json!([])),
        &original,
        &request
    ));
    assert!(item_moved(&moved(&|s| s["notes"] = json!(null)), &original, &request));
    let twice = parse({
        let mut raw = apply();
        raw["collectionIds"] = json!([OTHER, COLLECTION, OTHER]);
        raw
    })
    .unwrap();
    assert!(item_moved(
        &moved(&|s| s["collectionIds"] = json!([COLLECTION, OTHER])),
        &original,
        &twice
    ));
    assert!(!item_moved(
        &moved(&|s| s["collectionIds"] = json!([COLLECTION, OTHER, OTHER])),
        &original,
        &twice
    ));
}

#[test]
fn collection_verification_and_receipt_shape() {
    let create =
        parse(json!({"action": "create-collection", "organizationId": ORG, "operationId": OPERATION, "name": "Autre"}))
            .unwrap();
    let id = Uuid::parse(OTHER).unwrap();
    let mut orgs = organizations();
    assert!(!collection_created(&orgs, &create, &id));
    orgs[0]["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": OTHER, "name": "Autre"}));
    assert!(collection_created(&orgs, &create, &id));
    orgs[0]["collections"][1]["name"] = json!("Renommée");
    assert!(!collection_created(&orgs, &create, &id));
    let shown = serde_json::to_value(receipt(&create, &id).unwrap()).unwrap();
    assert_eq!(
        shown,
        json!({"id": OTHER, "operationId": OPERATION, "action": "create-collection", "verified": true, "organizationId": ORG})
    );
    let preview =
        parse(json!({"action": "preview", "itemId": ITEM, "organizationId": ORG, "collectionIds": [COLLECTION]}))
            .unwrap();
    assert_eq!(receipt(&preview, &id).unwrap_err(), ErrorCode::InvalidShare);
}
