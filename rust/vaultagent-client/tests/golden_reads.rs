//! Rejeu des cas dorés `rust/contrat/*.json` des routes de lecture contre le client : requête envoyée et sortie.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::rc::Rc;

use common::{TempDir, execute};
use serde_json::Value;
use vaultagent_client::fake_transport::FakeTransport;

const ITEM_ID: &str = "0000000a-0000-4000-8000-00000000000c";

const CASES: [&str; 10] = [
    "catalog-locked",
    "catalog-unlocked",
    "sharing-locked",
    "status-locked",
    "status-unlocked",
    "vaults",
    "write-preview-detail",
    "write-preview-list",
    "agent-unauthorized",
    "pair-request",
];

/// Charge un cas doré par son nom.
fn load(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contrat")
        .join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Remplace le gabarit de la fiche factice.
fn fill(value: &Value) -> Value {
    serde_json::from_str(&value.to_string().replace("{{itemId}}", ITEM_ID)).unwrap()
}

/// Arguments de la ligne de commande qui produisent la requête du cas.
fn arguments(path: &str, body: &Value) -> Vec<String> {
    let route = path.trim_start_matches("/api/agent/");
    let mut args = vec![route.replace("write-preview", "preview")];
    if let Some(search) = body["search"].as_str() {
        args.push(search.to_owned());
    }
    if let Some(id) = body["itemId"].as_str() {
        args.push(id.to_owned());
    }
    args
}

/// Vérifie que le corps attendu est inclus dans le corps envoyé.
fn assert_included(expected: &Value, sent: &Value) {
    for (key, value) in expected.as_object().unwrap() {
        assert_eq!(&sent[key], value, "key {key}");
    }
}

#[test]
fn read_cases_are_replayed_through_the_client() {
    let dir = TempDir::new();
    let config = dir.write_config();
    let config = config.to_str().unwrap();
    for name in CASES.iter().filter(|name| **name != "pair-request") {
        let case = load(name);
        let request = fill(&case["request"]);
        let response = &case["response"];
        let status = u16::try_from(response["status"].as_u64().unwrap()).unwrap();
        let body = response["body"].to_string();
        let fake = Rc::new(FakeTransport::new().reply(status, &body));
        let mut args = arguments(request["path"].as_str().unwrap(), &request["body"]);
        args.extend(["--config".to_owned(), config.to_owned()]);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let outcome = execute(&fake, &refs);
        let sent = &fake.calls()[0];
        assert_eq!(sent.path, request["path"].as_str().unwrap(), "{name}");
        assert_included(&request["body"], &serde_json::from_str(&sent.body).unwrap());
        if status == 200 {
            assert_eq!(outcome.exit.code(), 0, "{name}");
            assert_eq!(
                serde_json::from_str::<Value>(outcome.out.trim()).unwrap(),
                response["body"],
                "{name}"
            );
        } else {
            let expected = if status == 423 { 4 } else { 2 };
            assert_eq!(outcome.exit.code(), expected, "{name}");
            let line: Value = serde_json::from_str(outcome.err.trim()).unwrap();
            assert_eq!(line["error"], response["body"]["error"], "{name}");
        }
    }
}

#[test]
fn pair_request_body_matches_the_golden_case() {
    let case = load("pair-request");
    let expected = &case["request"]["body"];
    let dir = TempDir::new();
    let path = dir.path().join("client.json");
    let fake = Rc::new(FakeTransport::new().reply(400, "{\"error\":\"pairing-limit\"}"));
    let name = expected["name"].as_str().unwrap();
    execute(
        &fake,
        &[
            "pair-request",
            "https://vault.example.test",
            name,
            "--config",
            path.to_str().unwrap(),
        ],
    );
    let call = &fake.calls()[0];
    assert_eq!(call.path, case["request"]["path"].as_str().unwrap());
    assert_included(expected, &serde_json::from_str(&call.body).unwrap());
    assert!(call.bearer.is_none());
}
