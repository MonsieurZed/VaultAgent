//! Rejeu des cas dorés `write-*`, `request-unlock-*` et `lock` de `rust/contrat` contre le client.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::rc::Rc;

use common::{TempDir, execute_with_input};
use serde_json::{Map, Value};
use vaultagent_client::fake_transport::FakeTransport;

const OPERATION: &str = "0000000a-0000-4000-8000-00000000000d";
const ITEM: &str = "0000000a-0000-4000-8000-00000000000c";
const SECRET_KEYS: [&str; 4] = ["username", "password", "totp", "notes"];

/// Charge un cas doré et remplace ses gabarits.
fn load(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contrat")
        .join(format!("{name}.json"));
    let text = std::fs::read_to_string(path)
        .unwrap()
        .replace("{{operationId}}", OPERATION)
        .replace("{{itemId}}", ITEM);
    serde_json::from_str(&text).unwrap()
}

/// Sépare un objet en métadonnées et valeurs secrètes.
fn split(object: &Map<String, Value>) -> (Map<String, Value>, Map<String, Value>) {
    let mut meta = object.clone();
    let mut secrets = Map::new();
    for key in SECRET_KEYS {
        if let Some(value) = meta.remove(key) {
            secrets.insert(key.to_owned(), value);
        }
    }
    (meta, secrets)
}

/// Arguments et entrée standard qui produisent la requête du cas.
fn command_for(body: &Value) -> (Vec<String>, String) {
    let operation = ["--operation-id".to_owned(), OPERATION.to_owned()];
    let revision = body["expectedRevisionDate"].as_str().unwrap_or("").to_owned();
    match body["action"].as_str().unwrap() {
        "create" => {
            let mut item = body["item"].as_object().unwrap().clone();
            let mut login = item
                .remove("login")
                .and_then(|value| value.as_object().cloned())
                .unwrap_or_default();
            let (mut meta, mut secrets) = split(&item);
            let (login_rest, login_secrets) = split(&login);
            login = login_rest;
            secrets.extend(login_secrets);
            meta.insert("login".to_owned(), Value::Object(login));
            let args = [
                "create",
                "--metadata",
                &Value::Object(meta).to_string(),
                "--values-stdin",
            ];
            let mut all: Vec<String> = args.iter().map(ToString::to_string).collect();
            all.extend(operation);
            (all, Value::Object(secrets).to_string())
        }
        "update" => {
            let (meta, secrets) = split(body["patch"].as_object().unwrap());
            let mut all = vec![
                "update".to_owned(),
                ITEM.to_owned(),
                "--expected-revision".to_owned(),
                revision,
            ];
            all.extend(["--metadata".to_owned(), Value::Object(meta).to_string()]);
            all.extend(operation);
            (all, Value::Object(secrets).to_string())
        }
        "delete" => {
            let mut all = vec![
                "delete".to_owned(),
                ITEM.to_owned(),
                "--expected-revision".to_owned(),
                revision,
            ];
            all.extend(operation);
            (all, String::new())
        }
        "create-folder" => {
            let mut all = vec![
                "folder".to_owned(),
                "create".to_owned(),
                body["item"]["name"].as_str().unwrap().to_owned(),
            ];
            all.extend(operation);
            (all, String::new())
        }
        other => panic!("action not covered: {other}"),
    }
}

#[test]
fn write_cases_are_replayed_through_the_client() {
    let dir = TempDir::new();
    let config = dir.write_config();
    for name in ["write-create", "write-update", "write-delete", "write-create-folder"] {
        let case = load(name);
        let request = &case["request"]["body"];
        let reply = case["response"]["body"].to_string();
        let fake = Rc::new(FakeTransport::new().reply(200, &reply));
        let (mut args, stdin) = command_for(request);
        args.extend(["--config".to_owned(), config.to_str().unwrap().to_owned()]);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let outcome = execute_with_input(&fake, &refs, stdin.as_bytes());
        assert_eq!(outcome.exit.code(), 0, "{name}: {}", outcome.err);
        let sent: Value = serde_json::from_str(&fake.calls()[0].body).unwrap();
        assert_eq!(&sent, request, "{name}");
        assert_eq!(fake.calls()[0].path, case["request"]["path"].as_str().unwrap());
        assert_eq!(
            serde_json::from_str::<Value>(outcome.out.trim()).unwrap(),
            case["response"]["body"],
            "{name}"
        );
    }
}

#[test]
fn unlock_and_lock_cases_are_replayed_through_the_client() {
    let dir = TempDir::new();
    let config = dir.write_config();
    for (name, command) in [
        ("request-unlock-pending", "unlock"),
        ("request-unlock-unlocked", "unlock"),
        ("lock", "lock"),
    ] {
        let case = load(name);
        let response = case["response"]["body"].clone();
        let fake = Rc::new(FakeTransport::new().reply(200, &response.to_string()));
        let outcome = execute_with_input(&fake, &[command, "--config", config.to_str().unwrap()], b"");
        assert_eq!(
            fake.calls()[0].path,
            case["request"]["path"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(
            serde_json::from_str::<Value>(&fake.calls()[0].body).unwrap(),
            case["request"]["body"],
            "{name}"
        );
        let printed: Value = serde_json::from_str(outcome.out.trim()).unwrap();
        assert_eq!(printed, response, "{name}");
        let expected_exit = if response["unlocked"] == false || response["state"] == "pending" {
            4
        } else {
            0
        };
        assert_eq!(outcome.exit.code(), expected_exit, "{name}");
    }
}
