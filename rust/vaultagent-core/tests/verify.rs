//! Vérification par relecture : `comparable`, `matches_created`, `verify_update` et historique des mots de passe.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use std::time::Duration;
use vaultagent_core::verify::{ATTEMPTS, RETRY_DELAY, comparable, matches_created, verify_update};

/// Mutation d'une fiche.
type Fault = Box<dyn Fn(&mut Value)>;

const OLD_DATE: &str = "2026-10-01T00:00:00.000Z";

/// Fiche de référence avec un mot de passe et un historique de deux entrées.
fn base() -> Value {
    json!({"id": "00000000-0000-4000-8000-000000000001", "type": 1, "name": "Exemple factice",
        "folderId": null, "notes": null, "revisionDate": "2026-11-08T00:00:00.000Z",
        "attachments": [{"id": "a", "url": "https://example.test/a"}],
        "passwordHistory": [{"password": "Ancien-1", "lastUsedDate": OLD_DATE}],
        "login": {"username": "u@example.invalid", "password": "Fictif-1", "totp": null,
            "passwordRevisionDate": OLD_DATE,
            "uris": [{"uri": "https://example.test", "uriChecksum": "x"}]}})
}

/// Copie de la fiche modifiée par la fonction donnée.
fn changed(item: &Value, change: impl Fn(&mut Value)) -> Value {
    let mut copy = item.clone();
    change(&mut copy);
    copy
}

#[test]
fn retry_policy_constants() {
    assert_eq!(ATTEMPTS, 5);
    assert_eq!(RETRY_DELAY, Duration::from_millis(200));
}

#[test]
fn comparable_drops_server_maintained_fields() {
    let plain = comparable(&base(), false);
    assert!(plain.get("revisionDate").is_none());
    assert!(plain["attachments"][0].get("url").is_none());
    assert!(plain["login"]["uris"][0].get("uriChecksum").is_none());
    assert!(plain.get("passwordHistory").is_some());
    assert!(plain["login"].get("passwordRevisionDate").is_some());
    let renewed = comparable(&base(), true);
    assert!(renewed.get("passwordHistory").is_none());
    assert!(renewed["login"].get("passwordRevisionDate").is_none());
}

#[test]
fn matches_created_accepts_equivalent_items() {
    let item = json!({"type": 1, "name": "Exemple factice", "login": {"username": "u@example.invalid",
        "password": "Fictif-1", "uris": [{"uri": "https://example.test"}]}});
    let saved = changed(&base(), |s| s["login"]["totp"] = json!(""));
    assert!(matches_created(&saved, &item));
    let note = json!({"type": 2, "name": "Note", "notes": "x"});
    assert!(matches_created(
        &json!({"type": 2, "name": "Note", "notes": "x", "login": {"password": "ignoré"}}),
        &note
    ));
}

#[test]
fn matches_created_refuses_each_difference() {
    let item = json!({"type": 1, "name": "Exemple factice", "folderId": null, "login": {"username": "u@example.invalid",
        "password": "Fictif-1", "uris": [{"uri": "https://example.test"}]}});
    let faults: Vec<Fault> = vec![
        Box::new(|s| s["name"] = json!("Autre")),
        Box::new(|s| s["type"] = json!(2)),
        Box::new(|s| s["folderId"] = json!("00000000-0000-4000-8000-000000000007")),
        Box::new(|s| s["notes"] = json!("n")),
        Box::new(|s| s["organizationId"] = json!("00000000-0000-4000-8000-000000000008")),
        Box::new(|s| s["collectionIds"] = json!(["00000000-0000-4000-8000-000000000009"])),
        Box::new(|s| s["login"]["username"] = json!("autre")),
        Box::new(|s| s["login"]["password"] = json!("Fictif-2")),
        Box::new(|s| s["login"]["totp"] = json!("FAKE")),
        Box::new(|s| s["login"]["uris"] = json!([{"uri": "https://autre.test"}])),
        Box::new(|s| s["login"]["uris"] = json!([])),
    ];
    assert!(matches_created(&base(), &item));
    for fault in faults {
        assert!(!matches_created(&changed(&base(), |s| fault(s)), &item));
    }
}

#[test]
fn matches_created_ignores_collection_order() {
    let item = json!({"type": 2, "name": "N", "organizationId": "o", "collectionIds": ["b", "a"]});
    let saved = json!({"type": 2, "name": "N", "organizationId": "o", "collectionIds": ["a", "b"]});
    assert!(matches_created(&saved, &item));
}

#[test]
fn update_without_password_compares_whole_item() {
    let patch = json!({"name": "Nouveau"});
    let expected = changed(&base(), |i| i["name"] = json!("Nouveau"));
    let saved = changed(&expected, |i| i["revisionDate"] = json!("2026-11-09T00:00:00.000Z"));
    assert!(verify_update(&base(), &expected, &saved, &patch));
    assert!(!verify_update(&base(), &expected, &base(), &patch));
    let drifted = changed(&saved, |i| i["notes"] = json!("surprise"));
    assert!(!verify_update(&base(), &expected, &drifted, &patch));
}

#[test]
fn password_change_requires_old_password_first_in_history() {
    let patch = json!({"password": "Fictif-2"});
    let expected = changed(&base(), |i| i["login"]["password"] = json!("Fictif-2"));
    let saved = changed(&expected, |i| {
        i["passwordHistory"] = json!([
            {"password": "Fictif-1", "lastUsedDate": "2026-11-09T00:00:00.000Z"},
            {"password": "Ancien-1", "lastUsedDate": OLD_DATE}
        ]);
        i["login"]["passwordRevisionDate"] = json!("2026-11-09T00:00:00.000Z");
    });
    assert!(verify_update(&base(), &expected, &saved, &patch));
}

#[test]
fn password_history_failures_are_detected() {
    let patch = json!({"password": "Fictif-2"});
    let expected = changed(&base(), |i| i["login"]["password"] = json!("Fictif-2"));
    let with = |history: Value| changed(&expected, |i| i["passwordHistory"] = history.clone());
    let wrong_head = with(json!([{"password": "Autre"}, {"password": "Ancien-1", "lastUsedDate": OLD_DATE}]));
    let missing_tail = with(json!([{"password": "Fictif-1"}]));
    let empty = with(json!([]));
    let absent = changed(&expected, |i| {
        i.as_object_mut().unwrap().remove("passwordHistory");
    });
    for saved in [wrong_head, missing_tail, empty, absent] {
        assert!(!verify_update(&base(), &expected, &saved, &patch));
    }
}

#[test]
fn history_keeps_only_four_previous_entries() {
    let old: Vec<Value> = (0..6)
        .map(|n| json!({"password": format!("Ancien-{n}"), "lastUsedDate": "d"}))
        .collect();
    let before = changed(&base(), |i| i["passwordHistory"] = Value::Array(old.clone()));
    let expected = changed(&before, |i| i["login"]["password"] = json!("Fictif-2"));
    let mut kept = vec![json!({"password": "Fictif-1", "lastUsedDate": "n"})];
    kept.extend(old.iter().take(4).cloned());
    let saved = changed(&expected, |i| i["passwordHistory"] = Value::Array(kept.clone()));
    let patch = json!({"password": "Fictif-2"});
    assert!(verify_update(&before, &expected, &saved, &patch));
    kept.push(old[4].clone());
    let too_long = changed(&expected, |i| i["passwordHistory"] = Value::Array(kept.clone()));
    assert!(!verify_update(&before, &expected, &too_long, &patch));
}

#[test]
fn history_is_not_checked_without_previous_password_or_real_change() {
    let empty_before = changed(&base(), |i| {
        i["login"]["password"] = json!("");
        i.as_object_mut().unwrap().remove("passwordHistory");
    });
    let expected = changed(&empty_before, |i| i["login"]["password"] = json!("Fictif-2"));
    assert!(verify_update(
        &empty_before,
        &expected,
        &expected,
        &json!({"password": "Fictif-2"})
    ));
    let same = json!({"password": "Fictif-1"});
    assert!(verify_update(&base(), &base(), &base(), &same));
}
