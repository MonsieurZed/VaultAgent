//! Cycle de vie et créations : contrôles d'avant écriture et confirmation par relecture.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::creation_checks::{
    check_create_duplicate, check_create_folder, check_folder_reference, confirm_folder_created,
};
use vaultagent_core::lifecycle::{check_folder, check_item_lifecycle, confirm_lifecycle};
use vaultagent_core::{ErrorCode, Observed, WriteRequest};

const OP: &str = "00000000-0000-4000-8000-000000000002";
const ITEM: &str = "00000000-0000-4000-8000-000000000001";
const FOLDER: &str = "00000000-0000-4000-8000-000000000007";
const OTHER: &str = "00000000-0000-4000-8000-000000000006";
const ORG: &str = "00000000-0000-4000-8000-000000000008";
const CANARY: &str = "Fictif-SECRET";
const REV: &str = "2026-11-08T00:00:00.000Z";

/// Requête d'écriture validée.
fn request(body: Value) -> WriteRequest {
    WriteRequest::parse(&body).unwrap()
}

/// Requête de corbeille ou de restauration.
fn item_request(action: &str) -> WriteRequest {
    request(json!({"operationId": OP, "action": action, "itemId": ITEM, "expectedRevisionDate": REV}))
}

/// Fiche personnelle de référence.
fn item() -> Value {
    json!({"id": ITEM, "type": 1, "name": "Exemple factice", "revisionDate": REV, "deletedDate": null,
        "login": {"password": CANARY}})
}

/// Copie de la fiche modifiée.
fn with(change: impl Fn(&mut Value)) -> Value {
    let mut copy = item();
    change(&mut copy);
    copy
}

#[test]
fn delete_accepts_active_item_with_same_instant() {
    let other_format = with(|i| i["revisionDate"] = json!("2026-11-08T01:00:00.000+01:00"));
    assert_eq!(check_item_lifecycle(&item_request("delete"), &item()), Ok(None));
    assert_eq!(check_item_lifecycle(&item_request("delete"), &other_format), Ok(None));
}

#[test]
fn restore_needs_a_trashed_item_and_delete_an_active_one() {
    let trashed = with(|i| i["deletedDate"] = json!("2026-11-07T00:00:00.000Z"));
    assert_eq!(check_item_lifecycle(&item_request("restore"), &trashed), Ok(None));
    assert_eq!(
        check_item_lifecycle(&item_request("restore"), &item()),
        Err(ErrorCode::ItemRefused)
    );
    assert_eq!(
        check_item_lifecycle(&item_request("delete"), &trashed),
        Err(ErrorCode::ItemRefused)
    );
}

#[test]
fn lifecycle_refuses_stale_revision_unusable_item_and_wrong_owner() {
    let stale = with(|i| i["revisionDate"] = json!("2026-11-08T00:00:01.000Z"));
    assert_eq!(
        check_item_lifecycle(&item_request("delete"), &stale),
        Err(ErrorCode::RevisionConflict)
    );
    let unreadable = with(|i| i["revisionDate"] = json!("pas une date"));
    assert_eq!(
        check_item_lifecycle(&item_request("delete"), &unreadable),
        Err(ErrorCode::RequestRefused)
    );
    for bad in [
        with(|i| i["archivedDate"] = json!("2026-01-01T00:00:00.000Z")),
        with(|i| i["type"] = json!(3)),
        with(|i| i["id"] = json!("x")),
        with(|i| i["organizationId"] = json!(ORG)),
        with(|i| i["organizationId"] = json!(5)),
    ] {
        assert_eq!(
            check_item_lifecycle(&item_request("delete"), &bad),
            Err(ErrorCode::ItemRefused)
        );
    }
}

#[test]
fn shared_item_returns_its_sharing_scope_and_needs_matching_organization() {
    let shared = with(|i| {
        i["organizationId"] = json!(ORG);
        i["collectionIds"] = json!([OTHER]);
    });
    let personal = item_request("delete");
    assert_eq!(check_item_lifecycle(&personal, &shared), Err(ErrorCode::ItemRefused));
    let in_org = request(
        json!({"operationId": OP, "action": "delete", "itemId": ITEM, "expectedRevisionDate": REV, "organizationId": ORG}),
    );
    let scope = check_item_lifecycle(&in_org, &shared).unwrap().unwrap();
    assert_eq!(scope.organization_id, ORG);
    assert_eq!(scope.collection_ids, [OTHER]);
    assert_eq!(check_item_lifecycle(&in_org, &item()), Err(ErrorCode::ItemRefused));
}

#[test]
fn lifecycle_check_refuses_other_actions() {
    let folder = request(json!({"operationId": OP, "action": "delete-folder", "folderId": FOLDER}));
    assert_eq!(check_item_lifecycle(&folder, &item()), Err(ErrorCode::InvalidWrite));
    assert_eq!(check_folder(&item_request("delete"), &[]), Err(ErrorCode::InvalidWrite));
}

#[test]
fn folder_checks() {
    let folders = [
        json!({"id": FOLDER, "name": "Ancien"}),
        json!({"id": OTHER, "name": "Pris"}),
    ];
    let rename = |id: &str, name: &str| {
        request(json!({"operationId": OP, "action": "update-folder", "folderId": id, "item": {"name": name}}))
    };
    assert_eq!(check_folder(&rename(FOLDER, "Libre"), &folders), Ok(()));
    assert_eq!(check_folder(&rename(FOLDER, "Ancien"), &folders), Ok(()));
    assert_eq!(
        check_folder(&rename(FOLDER, "Pris"), &folders),
        Err(ErrorCode::DuplicateFolder)
    );
    assert_eq!(
        check_folder(&rename(ITEM, "Libre"), &folders),
        Err(ErrorCode::FolderMissing)
    );
    let delete = request(json!({"operationId": OP, "action": "delete-folder", "folderId": FOLDER}));
    assert_eq!(check_folder(&delete, &folders), Ok(()));
    assert_eq!(check_folder(&delete, &folders[1..]), Err(ErrorCode::FolderMissing));
}

#[test]
fn confirm_item_lifecycle_reads_the_trash_state() {
    let trashed = with(|i| i["deletedDate"] = json!("2026-11-09T00:00:00.000Z"));
    let receipt = confirm_lifecycle(&item_request("delete"), Observed::Item(&trashed))
        .unwrap()
        .unwrap();
    assert_eq!(
        (receipt.id.as_str(), receipt.action.as_str(), receipt.verified),
        (ITEM, "delete", true)
    );
    assert_eq!(receipt.operation_id, OP);
    assert_eq!(receipt.revision_date.as_deref(), Some(REV));
    assert_eq!(
        confirm_lifecycle(&item_request("delete"), Observed::Item(&item())),
        Ok(None)
    );
    assert!(
        confirm_lifecycle(&item_request("restore"), Observed::Item(&item()))
            .unwrap()
            .is_some()
    );
    assert_eq!(
        confirm_lifecycle(&item_request("restore"), Observed::Item(&trashed)),
        Ok(None)
    );
    let archived = with(|i| i["archivedDate"] = json!("x"));
    assert_eq!(
        confirm_lifecycle(&item_request("delete"), Observed::Item(&archived)),
        Err(ErrorCode::ItemRefused)
    );
}

#[test]
fn confirm_receipt_holds_no_secret() {
    let trashed = with(|i| i["deletedDate"] = json!("2026-11-09T00:00:00.000Z"));
    let receipt = confirm_lifecycle(&item_request("delete"), Observed::Item(&trashed))
        .unwrap()
        .unwrap();
    assert!(!serde_json::to_string(&receipt).unwrap().contains(CANARY));
}

#[test]
fn confirm_folder_lifecycle() {
    let folders = [json!({"id": FOLDER, "name": "Nouveau"})];
    let rename =
        request(json!({"operationId": OP, "action": "update-folder", "folderId": FOLDER, "item": {"name": "Nouveau"}}));
    let receipt = confirm_lifecycle(&rename, Observed::Folders(&folders))
        .unwrap()
        .unwrap();
    assert_eq!((receipt.id.as_str(), receipt.revision_date), (FOLDER, None));
    let stale = [json!({"id": FOLDER, "name": "Ancien"})];
    assert_eq!(confirm_lifecycle(&rename, Observed::Folders(&stale)), Ok(None));
    let delete = request(json!({"operationId": OP, "action": "delete-folder", "folderId": FOLDER}));
    assert_eq!(confirm_lifecycle(&delete, Observed::Folders(&folders)), Ok(None));
    assert!(confirm_lifecycle(&delete, Observed::Folders(&[])).unwrap().is_some());
    assert_eq!(
        confirm_lifecycle(&delete, Observed::Item(&item())),
        Err(ErrorCode::InvalidWrite)
    );
}

#[test]
fn create_duplicate_compares_name_folder_and_organization() {
    let create = request(
        json!({"operationId": OP, "action": "create", "item": {"type": 2, "name": "Note", "folderId": FOLDER}}),
    );
    let same = json!({"name": "Note", "folderId": FOLDER, "organizationId": null});
    assert_eq!(
        check_create_duplicate(&create, std::slice::from_ref(&same)),
        Err(ErrorCode::DuplicateItem)
    );
    let trashed = json!({"name": "Note", "folderId": FOLDER, "deletedDate": "x"});
    let elsewhere = json!({"name": "Note", "folderId": null});
    let shared = json!({"name": "Note", "folderId": FOLDER, "organizationId": ORG});
    let renamed = json!({"name": "Autre", "folderId": FOLDER});
    assert_eq!(
        check_create_duplicate(&create, &[trashed, elsewhere, shared, renamed]),
        Ok(())
    );
    assert_eq!(
        check_create_duplicate(&item_request("delete"), &[]),
        Err(ErrorCode::InvalidWrite)
    );
}

#[test]
fn create_folder_resumes_or_refuses_duplicates() {
    let create = request(json!({"operationId": OP, "action": "create-folder", "item": {"name": "Neuf"}}));
    assert_eq!(
        check_create_folder(&create, &[json!({"id": FOLDER, "name": "Autre"})]),
        Ok(None)
    );
    let one = [json!({"id": FOLDER, "name": "Neuf"})];
    assert_eq!(check_create_folder(&create, &one), Ok(Some(FOLDER.to_owned())));
    let two = [one[0].clone(), json!({"id": OTHER, "name": "Neuf"})];
    assert_eq!(check_create_folder(&create, &two), Err(ErrorCode::DuplicateFolder));
    assert_eq!(
        confirm_folder_created(&item_request("delete"), FOLDER, &one),
        Err(ErrorCode::InvalidWrite)
    );
    let receipt = confirm_folder_created(&create, FOLDER, &one).unwrap().unwrap();
    assert_eq!(
        (receipt.action.as_str(), receipt.id.as_str()),
        ("create-folder", FOLDER)
    );
    assert_eq!(confirm_folder_created(&create, OTHER, &one), Ok(None));
}

#[test]
fn folder_reference_must_exist_when_given() {
    let folders = [json!({"id": FOLDER, "name": "X"})];
    assert_eq!(check_folder_reference(None, &folders), Ok(()));
    assert_eq!(check_folder_reference(Some(&json!(null)), &folders), Ok(()));
    assert_eq!(check_folder_reference(Some(&json!(FOLDER)), &folders), Ok(()));
    assert_eq!(
        check_folder_reference(Some(&json!(OTHER)), &folders),
        Err(ErrorCode::FolderMissing)
    );
}
