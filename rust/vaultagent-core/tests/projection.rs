//! Projections sans secret et contrôles de visibilité d'une fiche.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::{ErrorCode, active, catalog_entry, item_detail, item_metadata, visible};

const ID: &str = "00000000-0000-4000-8000-000000000001";
const DUMMY: &str = "Fictif-seulement-42!";

/// Connexion factice portant un secret reconnaissable.
fn login_item() -> Value {
    json!({"id": ID, "name": "Compte", "type": 1, "folderId": "00000000-0000-4000-8000-000000000007",
        "revisionDate": "2026-11-08T00:00:00.000Z", "organizationId": "00000000-0000-4000-8000-000000000008",
        "collectionIds": ["00000000-0000-4000-8000-000000000009"], "notes": DUMMY,
        "login": {"username": "u@example.invalid", "password": DUMMY, "totp": "FAKESEED", "uris": [
            {"uri": "https://Example.test/a"}, {"uri": "https://example.test/b?x=1"}, {"uri": "http://plain.test"},
            {"uri": "https://other.test:8443"}, {"match": null}]}})
}

#[test]
fn catalog_entry_has_presence_and_distinct_origins() {
    let entry = catalog_entry(&login_item()).unwrap();
    assert_eq!(entry.origins, vec!["https://example.test", "https://other.test:8443"]);
    assert!(entry.username && entry.password && entry.totp);
    let value = serde_json::to_value(&entry).unwrap();
    assert_eq!(value["organizationId"], "00000000-0000-4000-8000-000000000008");
    assert_eq!(value["collectionIds"], json!(["00000000-0000-4000-8000-000000000009"]));
    assert!(!value.to_string().contains(DUMMY));
}

#[test]
fn catalog_skips_notes_and_reports_absent_values() {
    assert!(catalog_entry(&json!({"id": ID, "type": 2, "name": "Note"})).is_none());
    let bare =
        catalog_entry(&json!({"id": ID, "type": 1, "name": "Vide", "login": {"username": "", "uris": null}})).unwrap();
    assert!(!bare.username && !bare.password && !bare.totp);
    assert!(bare.origins.is_empty() && bare.collection_ids.is_empty() && bare.organization_id.is_none());
}

#[test]
fn metadata_has_exact_protocol_fields() {
    let value = serde_json::to_value(item_metadata(&login_item()).unwrap()).unwrap();
    let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
    let mut expected = vec![
        "id",
        "name",
        "type",
        "folderId",
        "revisionDate",
        "organizationId",
        "collectionIds",
    ];
    expected.sort_unstable();
    assert_eq!(
        keys.iter().copied().collect::<std::collections::BTreeSet<_>>(),
        expected.into_iter().collect()
    );
    assert_eq!(value["type"], 1);
    assert_eq!(value["folderId"], "00000000-0000-4000-8000-000000000007");
}

#[test]
fn detail_has_presence_deleted_and_uris_without_secrets() {
    let mut item = login_item();
    item["deletedDate"] = json!("2026-11-08T00:00:00.000Z");
    let detail = item_detail(&item).unwrap();
    assert!(detail.deleted);
    assert_eq!(detail.uris.len(), 4);
    assert!(detail.has.username && detail.has.password && detail.has.totp && detail.has.notes);
    let value = serde_json::to_value(&detail).unwrap();
    assert_eq!(
        value["has"],
        json!({"username": true, "password": true, "totp": true, "notes": true})
    );
    assert_eq!(value["id"], ID);
    assert!(!value.to_string().contains(DUMMY));
}

#[test]
fn detail_of_a_note_has_no_login_presence() {
    let detail = item_detail(&json!({"id": ID, "type": 2, "name": "Note", "revisionDate": "r", "notes": ""})).unwrap();
    assert!(!detail.has.notes && !detail.has.password && !detail.deleted && detail.uris.is_empty());
    assert!(item_detail(&json!({"name": "sans id"})).is_none());
}

#[test]
fn visible_refuses_archived_unknown_type_and_bad_id() {
    assert!(visible(&login_item()).is_ok());
    let mut trashed = login_item();
    trashed["deletedDate"] = json!("2026-11-08T00:00:00.000Z");
    assert!(visible(&trashed).is_ok());
    let mut archived = login_item();
    archived["archivedDate"] = json!("2026-11-08T00:00:00.000Z");
    let mut card = login_item();
    card["type"] = json!(3);
    let mut bad_id = login_item();
    bad_id["id"] = json!("nope");
    for item in [archived, card, bad_id, json!("x"), json!(null)] {
        assert_eq!(visible(&item), Err(ErrorCode::ItemRefused), "{item}");
    }
}

#[test]
fn active_also_refuses_trash() {
    assert!(active(&login_item()).is_ok());
    let mut trashed = login_item();
    trashed["deletedDate"] = json!("2026-11-08T00:00:00.000Z");
    assert_eq!(active(&trashed), Err(ErrorCode::ItemRefused));
}
