//! Cas dorés de `rust/contrat` : les requêtes d'écriture sont acceptées ou refusées avec le code du cas, et les
//! projections reproduisent les corps de réponse.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::{ErrorCode, WriteAction, WriteRequest, catalog_entry, item_detail, item_metadata};

const OPERATION_ID: &str = "00000000-0000-4000-8000-000000000002";
const ITEM_ID: &str = "00000000-0000-4000-8000-000000000001";

/// Lit un cas doré en remplaçant les gabarits `{{operationId}}` et `{{itemId}}`.
fn golden(source: &str) -> Value {
    let filled = source
        .replace("{{operationId}}", OPERATION_ID)
        .replace("{{itemId}}", ITEM_ID);
    serde_json::from_str(&filled).unwrap()
}

/// Item de coffre factice correspondant aux cas dorés de l'aperçu et du catalogue.
fn vault_item() -> Value {
    json!({
        "id": ITEM_ID, "name": "Compte factice", "type": 1, "folderId": null,
        "revisionDate": "2026-11-08T00:00:00.000Z", "organizationId": null, "collectionIds": [],
        "login": {"username": "u@example.invalid", "password": "Fictif-1", "totp": "FAKESEED",
                  "uris": [{"uri": "https://example.test/login", "match": null}]}
    })
}

#[test]
fn golden_create_is_accepted() {
    let case = golden(include_str!("../../contrat/write-create.json"));
    let request = WriteRequest::parse(&case["request"]["body"]).unwrap();
    assert!(matches!(request.action(), WriteAction::Create { .. }));
    assert_eq!(request.raw(), &case["request"]["body"]);
    assert_eq!(request.operation_id().as_str(), OPERATION_ID);
}

#[test]
fn golden_invalid_write_is_refused_with_the_case_code() {
    let case = golden(include_str!("../../contrat/write-invalid-write.json"));
    let expected = ErrorCode::parse(case["response"]["body"]["error"].as_str().unwrap());
    assert_eq!(WriteRequest::parse(&case["request"]["body"]).unwrap_err(), expected);
    assert_eq!(
        u64::from(expected.http_status()),
        case["response"]["status"].as_u64().unwrap()
    );
}

#[test]
fn golden_update_delete_and_folder_requests_are_accepted() {
    let update = golden(include_str!("../../contrat/write-update.json"));
    let delete = golden(include_str!("../../contrat/write-delete.json"));
    let folder = golden(include_str!("../../contrat/write-create-folder.json"));
    assert!(matches!(
        WriteRequest::parse(&update["request"]["body"]).unwrap().action(),
        WriteAction::Update { .. }
    ));
    assert!(matches!(
        WriteRequest::parse(&delete["request"]["body"]).unwrap().action(),
        WriteAction::Delete { .. }
    ));
    assert!(matches!(
        WriteRequest::parse(&folder["request"]["body"]).unwrap().action(),
        WriteAction::CreateFolder { .. }
    ));
}

#[test]
fn golden_preview_detail_matches_projection() {
    let case = golden(include_str!("../../contrat/write-preview-detail.json"));
    let detail = item_detail(&vault_item()).unwrap();
    assert_eq!(serde_json::to_value(detail).unwrap(), case["response"]["body"]);
}

#[test]
fn golden_preview_list_item_matches_metadata() {
    let case = golden(include_str!("../../contrat/write-preview-list.json"));
    let metadata = item_metadata(&vault_item()).unwrap();
    assert_eq!(
        serde_json::to_value(metadata).unwrap(),
        case["response"]["body"]["items"][0]
    );
}

#[test]
fn golden_catalog_matches_projection() {
    let case = golden(include_str!("../../contrat/catalog-unlocked.json"));
    let entry = catalog_entry(&vault_item()).unwrap();
    assert_eq!(serde_json::to_value(entry).unwrap(), case["response"]["body"][0]);
}
