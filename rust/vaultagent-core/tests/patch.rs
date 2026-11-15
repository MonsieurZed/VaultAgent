//! Patch d'écriture : chaque branche de `fork/write-patch.mjs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::{ErrorCode, RevisionDate, Uuid, apply_write_patch};

const REV: &str = "2026-11-08T00:00:00.000Z";
const ORG: &str = "00000000-0000-4000-8000-000000000008";
const FOLDER: &str = "00000000-0000-4000-8000-000000000007";

/// Date de révision de référence.
fn rev() -> RevisionDate {
    RevisionDate::parse(REV).unwrap()
}

/// Connexion factice avec champs que le patch ne doit pas toucher.
fn view() -> Value {
    json!({"type": 1, "name": "Compte", "notes": "n", "folderId": null, "revisionDate": REV, "organizationId": null,
        "fields": [{"name": "Secret", "value": "dummy-field"}], "attachments": [{"id": "dummy-attachment"}],
        "login": {"username": "u", "password": "p", "totp": "t", "fido2Credentials": [{"keyValue": "dummy-key"}],
            "autofillOnPageLoad": true, "uris": [{"uri": "https://example.test", "match": 1, "uriChecksum": "dummy"}]}})
}

/// Applique un patch à la fiche de référence.
fn apply(patch: Value) -> Result<Value, ErrorCode> {
    let mut item = view();
    apply_write_patch(&mut item, &rev(), &patch, None).map(|()| item)
}

/// Applique un patch et attend l'erreur donnée.
fn fails(patch: Value, code: &ErrorCode) {
    let mut item = view();
    assert_eq!(
        apply_write_patch(&mut item, &rev(), &patch, None),
        Err(code.clone()),
        "{patch}"
    );
    assert_eq!(item, view(), "la fiche doit rester inchangée");
}

#[test]
fn scalar_fields_are_applied_and_others_kept() {
    let item = apply(json!({"name": "Renommé", "notes": "", "folderId": FOLDER, "password": "nouveau", "username": "v", "totp": "T"})).unwrap();
    assert_eq!(item["name"], "Renommé");
    assert_eq!(item["notes"], "");
    assert_eq!(item["folderId"], FOLDER);
    assert_eq!(item["login"]["password"], "nouveau");
    assert_eq!(item["login"]["username"], "v");
    assert_eq!(item["login"]["totp"], "T");
    let original = view();
    assert_eq!(item["fields"], original["fields"]);
    assert_eq!(item["attachments"], original["attachments"]);
    assert_eq!(item["login"]["fido2Credentials"], original["login"]["fido2Credentials"]);
    assert_eq!(item["login"]["autofillOnPageLoad"], true);
    assert_eq!(item["login"]["uris"], original["login"]["uris"]);
}

#[test]
fn folder_can_be_cleared() {
    let mut item = view();
    item["folderId"] = json!(FOLDER);
    apply_write_patch(&mut item, &rev(), &json!({"folderId": null}), None).unwrap();
    assert_eq!(item["folderId"], Value::Null);
}

#[test]
fn invalid_patch_shapes_and_values() {
    for patch in [
        json!({}),
        json!(null),
        json!([]),
        json!("x"),
        json!({"fields": []}),
        json!({"name": " "}),
        json!({"name": "a\nb"}),
        json!({"name": "a".repeat(501)}),
        json!({"name": 4}),
        json!({"notes": "n".repeat(12_001)}),
        json!({"notes": null}),
        json!({"username": "u".repeat(8193)}),
        json!({"password": 1}),
        json!({"folderId": "nope"}),
        json!({"folderId": 5}),
    ] {
        fails(patch, &ErrorCode::InvalidPatch);
    }
    assert!(apply(json!({"notes": "n".repeat(12_000), "password": "p".repeat(8192), "name": "a".repeat(500)})).is_ok());
}

#[test]
fn revision_must_match_by_instant() {
    let mut item = view();
    let other = RevisionDate::parse("2026-11-08T00:00:01.000Z").unwrap();
    assert_eq!(
        apply_write_patch(&mut item, &other, &json!({"name": "x"}), None),
        Err(ErrorCode::RevisionConflict)
    );
    let same = RevisionDate::parse("2026-11-08T01:00:00.000+01:00").unwrap();
    assert!(apply_write_patch(&mut item, &same, &json!({"name": "x"}), None).is_ok());
    let mut broken = view();
    broken["revisionDate"] = json!("pas une date");
    assert_eq!(
        apply_write_patch(&mut broken, &rev(), &json!({"name": "x"}), None),
        Err(ErrorCode::RequestRefused)
    );
}

#[test]
fn item_state_and_owner_are_checked() {
    let org = Uuid::parse(ORG).unwrap();
    let mut shared = view();
    shared["organizationId"] = json!(ORG);
    assert_eq!(
        apply_write_patch(&mut shared, &rev(), &json!({"name": "x"}), None),
        Err(ErrorCode::ItemRefused)
    );
    assert!(apply_write_patch(&mut shared, &rev(), &json!({"name": "x"}), Some(&org)).is_ok());
    let mut personal = view();
    assert_eq!(
        apply_write_patch(&mut personal, &rev(), &json!({"name": "x"}), Some(&org)),
        Err(ErrorCode::ItemRefused)
    );
    for (key, value) in [
        ("isDeleted", json!(true)),
        ("isArchived", json!(true)),
        ("deletedDate", json!(REV)),
        ("archivedDate", json!(REV)),
        ("type", json!(3)),
    ] {
        let mut item = view();
        item[key] = value;
        assert_eq!(
            apply_write_patch(&mut item, &rev(), &json!({"name": "x"}), None),
            Err(ErrorCode::ItemRefused),
            "{key}"
        );
    }
    let mut not_object = json!("x");
    assert_eq!(
        apply_write_patch(&mut not_object, &rev(), &json!({"name": "x"}), None),
        Err(ErrorCode::ItemRefused)
    );
}

#[test]
fn login_fields_only_for_type_one() {
    let mut note = json!({"type": 2, "name": "Note", "revisionDate": REV, "organizationId": null});
    for field in ["username", "password", "totp"] {
        assert_eq!(
            apply_write_patch(&mut note, &rev(), &json!({field: "v"}), None),
            Err(ErrorCode::ItemRefused)
        );
    }
    assert!(apply_write_patch(&mut note, &rev(), &json!({"name": "Renommée", "notes": "x"}), None).is_ok());
    let mut no_login = json!({"type": 1, "revisionDate": REV, "organizationId": null});
    assert_eq!(
        apply_write_patch(&mut no_login, &rev(), &json!({"password": "v"}), None),
        Err(ErrorCode::ItemRefused)
    );
}

#[test]
fn add_uris_appends_new_entries_without_touching_existing() {
    let item =
        apply(json!({"addUris": ["https://nvr.example.test", "https://example.test", "https://nvr.example.test"]}))
            .unwrap();
    let uris = item["login"]["uris"].as_array().unwrap();
    assert_eq!(uris.len(), 2);
    assert_eq!(uris[0], view()["login"]["uris"][0]);
    assert_eq!(uris[1], json!({"uri": "https://nvr.example.test", "match": null}));
    assert_eq!(item["login"]["password"], "p");
}

#[test]
fn add_uris_refusals() {
    for value in [
        json!(["http://nvr.example.test"]),
        json!(["https://user:pass@nvr.example.test"]),
        json!(["https://nvr.example.test/?a=1"]),
        json!(["https://nvr.example.test/#x"]),
        json!(["nvr"]),
        json!([]),
        json!("https://nvr.example.test"),
        json!([1]),
        json!(
            (0..21)
                .map(|n| format!("https://h{n}.example.test"))
                .collect::<Vec<_>>()
        ),
    ] {
        fails(json!({"addUris": value}), &ErrorCode::InvalidPatch);
    }
    let mut full = view();
    full["login"]["uris"] = json!(
        (0..20)
            .map(|n| json!({"uri": format!("https://h{n}.example.test")}))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        apply_write_patch(
            &mut full,
            &rev(),
            &json!({"addUris": ["https://extra.example.test"]}),
            None
        ),
        Err(ErrorCode::InvalidPatch)
    );
    assert!(
        apply_write_patch(
            &mut full,
            &rev(),
            &json!({"addUris": ["https://h3.example.test"]}),
            None
        )
        .is_ok()
    );
    let mut note = json!({"type": 2, "revisionDate": REV, "organizationId": null});
    assert_eq!(
        apply_write_patch(
            &mut note,
            &rev(),
            &json!({"addUris": ["https://ok.example.test"]}),
            None
        ),
        Err(ErrorCode::InvalidPatch)
    );
}

#[test]
fn add_uris_creates_list_when_absent() {
    let mut item = json!({"type": 1, "revisionDate": REV, "organizationId": null, "login": {"uris": null}});
    apply_write_patch(
        &mut item,
        &rev(),
        &json!({"addUris": ["https://ok.example.test"]}),
        None,
    )
    .unwrap();
    assert_eq!(
        item["login"]["uris"],
        json!([{"uri": "https://ok.example.test", "match": null}])
    );
}

#[test]
fn remove_uris_keeps_other_entries_and_requires_presence() {
    let mut item = view();
    item["login"]["uris"] = json!([{"uri": "https://a.example.test", "match": 2}, {"uri": "http://legacy.example.test"}, {"uri": "https://b.example.test"}]);
    apply_write_patch(
        &mut item,
        &rev(),
        &json!({"removeUris": ["http://legacy.example.test"]}),
        None,
    )
    .unwrap();
    assert_eq!(
        item["login"]["uris"],
        json!([{"uri": "https://a.example.test", "match": 2}, {"uri": "https://b.example.test"}])
    );
    fails(
        json!({"removeUris": ["https://absent.example.test"]}),
        &ErrorCode::InvalidPatch,
    );
    fails(json!({"removeUris": []}), &ErrorCode::InvalidPatch);
    fails(json!({"removeUris": [""]}), &ErrorCode::InvalidPatch);
    fails(json!({"removeUris": "https://example.test"}), &ErrorCode::InvalidPatch);
}

#[test]
fn remove_runs_before_add_to_replace_an_address() {
    let item = apply(json!({"removeUris": ["https://example.test"], "addUris": ["https://example.test", "https://new.example.test"]})).unwrap();
    assert_eq!(
        item["login"]["uris"],
        json!([
            {"uri": "https://example.test", "match": null}, {"uri": "https://new.example.test", "match": null}
        ])
    );
}

#[test]
fn failure_after_partial_application_leaves_item_untouched() {
    fails(
        json!({"addUris": ["https://new.example.test"], "name": " "}),
        &ErrorCode::InvalidPatch,
    );
    fails(json!({"name": "ok", "folderId": "nope"}), &ErrorCode::InvalidPatch);
}
