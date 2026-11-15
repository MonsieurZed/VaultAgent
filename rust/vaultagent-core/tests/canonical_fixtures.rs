//! Vecteurs générés par Node (`JSON.stringify(canonical(...))`, HMAC) : la sérialisation canonique, l'empreinte et
//! l'aller-retour des registres écrits par Node sont identiques.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::Value;
use vaultagent_core::{Ledger, Registry, canonical_json, fingerprint};

const FIXTURE: &str = include_str!("fixtures/canonical.json");

/// Fixture complète lue une fois.
fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

/// Clé factice `00 01 .. 1f`.
fn dummy_key() -> [u8; 32] {
    std::array::from_fn(|index| u8::try_from(index).unwrap())
}

#[test]
fn canonical_vectors_match_node_output() {
    let data = fixture();
    let cases = data["canonical"].as_array().unwrap();
    assert!(cases.len() >= 15);
    for case in cases {
        let input: Value = serde_json::from_str(case["input"].as_str().unwrap()).unwrap();
        assert_eq!(
            canonical_json(&input),
            case["expected"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn fingerprints_match_node_hmac() {
    let data = fixture();
    let key = dummy_key();
    for case in data["fingerprints"].as_array().unwrap() {
        assert_eq!(case["key_hex"].as_str().unwrap().len(), 64);
        let request: Value = serde_json::from_str(case["input"].as_str().unwrap()).unwrap();
        let print = fingerprint(&key, &request);
        assert_eq!(print, case["expected"].as_str().unwrap());
        assert_eq!(print.len(), 64);
        assert!(print.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    }
}

#[test]
fn fingerprint_ignores_key_order_and_nulls() {
    let key = dummy_key();
    let first: Value = serde_json::from_str(r#"{"a":1,"b":null,"c":{"x":1,"y":2}}"#).unwrap();
    let second: Value = serde_json::from_str(r#"{"c":{"y":2,"x":1},"a":1}"#).unwrap();
    assert_eq!(fingerprint(&key, &first), fingerprint(&key, &second));
    let other: Value = serde_json::from_str(r#"{"a":2,"c":{"y":2,"x":1}}"#).unwrap();
    assert_ne!(fingerprint(&key, &first), fingerprint(&key, &other));
}

#[test]
fn node_written_registries_round_trip() {
    let data = fixture();
    for field in ["writeLedger", "shareLedger"] {
        let text = data[field].as_str().unwrap();
        let ledger: Ledger = serde_json::from_str(text).unwrap();
        assert!(!ledger.is_empty());
        let rewritten = serde_json::to_value(&ledger).unwrap();
        assert_eq!(rewritten, serde_json::from_str::<Value>(text).unwrap(), "{field}");
    }
}

#[test]
fn write_ledger_from_node_has_receipts_and_an_intent() {
    let data = fixture();
    let ledger: Ledger = serde_json::from_str(data["writeLedger"].as_str().unwrap()).unwrap();
    let with_receipt = ledger.values().filter(|entry| entry.receipt.is_some()).count();
    assert_eq!(with_receipt, 2);
    assert_eq!(ledger.len() - with_receipt, 1);
    let share: Ledger = serde_json::from_str(data["shareLedger"].as_str().unwrap()).unwrap();
    let receipt = share.values().next().unwrap().receipt.as_ref().unwrap();
    assert_eq!(
        receipt.organization_id.as_deref(),
        Some("00000000-0000-4000-8000-000000000008")
    );
}

#[test]
fn registry_file_names_match_node() {
    assert_eq!(Registry::Write.file_name(), "write-receipts.json");
    assert_eq!(Registry::Share.file_name(), "share-receipts.json");
}
