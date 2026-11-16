//! Rejeu des 49 cas dorés `rust/contrat/*.json` (agent, association, administration, front, téléchargements,
//! `/healthz`) contre `route()` et le faux moteur.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;
mod golden {
    pub mod compare;
    pub mod scripts;
    pub mod template;
}

use std::path::Path;

use common::service::{Client, Service, VaultState};
use golden::compare::compare;
use golden::scripts::replies;
use golden::template::{Values, substitute};
use serde_json::{Map, Value, json};
use vaultagent_server::Request;

/// Cas chargés depuis `rust/contrat`, triés par nom.
fn cases() -> Vec<Value> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../contrat");
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
        .collect()
}

/// État du coffre et du client d'un cas.
fn service_for(case: &Value) -> Service {
    let fixture = &case["fixture"];
    let state = match fixture["vault"].as_str() {
        Some("unconfigured") => VaultState::Unconfigured,
        Some("locked") => VaultState::Locked,
        _ => VaultState::Unlocked,
    };
    let client = match fixture["client"].as_str() {
        Some("none") => Client::None,
        Some("approver") => Client::Approver,
        _ => Client::Paired,
    };
    Service::new(state, client)
}

/// Construit la requête d'une étape après substitution des gabarits.
fn build(step: &Value, values: &Values, remote: &str) -> Request {
    let request = substitute(&step["request"], values);
    let method = request["method"].as_str().unwrap();
    let mut built = Request::new(method, request["path"].as_str().unwrap(), remote);
    if let Some(headers) = request["headers"].as_object() {
        for (name, value) in headers {
            built = built.with_header(name, value.as_str().unwrap());
        }
    }
    if let Some(raw) = request["rawBody"].as_str() {
        return built.with_body(raw.as_bytes());
    }
    if request.get("body").is_none() {
        return built;
    }
    let mut body = request["body"].clone();
    if let (Some(padding), Some(map)) = (request["bodyPadding"].as_u64(), body.as_object_mut()) {
        map.insert(
            "padding".to_owned(),
            Value::String("x".repeat(usize::try_from(padding).unwrap())),
        );
    }
    built.with_body(body.to_string().as_bytes())
}

/// Rejoue un cas et rend l'écart éventuel.
fn replay(case: &Value) -> Result<(), String> {
    let service = service_for(case);
    service.script(replies(case["name"].as_str().unwrap()));
    let remote = if case["fixture"]["proxy"] == "untrusted" {
        "10.0.0.9"
    } else {
        "::ffff:127.0.0.1"
    };
    let steps: Vec<Value> = match case.get("steps") {
        Some(list) => list.as_array().unwrap().clone(),
        None => vec![json!({"request": case["request"], "response": case["response"]})],
    };
    let mut values = Values {
        token: service.token.clone().unwrap_or_default(),
        steps: Vec::new(),
    };
    for (index, step) in steps.iter().enumerate() {
        let response = service.send(&build(step, &values, remote));
        let want = &step["response"];
        let label = format!("étape {}", index + 1);
        if u64::from(response.status) != want["status"].as_u64().unwrap() {
            return Err(format!(
                "{label}: statut {} reçu, {} attendu ({:?})",
                response.status,
                want["status"],
                response.body_json()
            ));
        }
        let headers: &Map<String, Value> = want["headers"].as_object().unwrap();
        for (name, value) in headers {
            if response.header(name) != value.as_str() {
                return Err(format!(
                    "{label}: en-tête {name} {:?}, attendu {value}",
                    response.header(name)
                ));
            }
        }
        if want.get("body").is_none() {
            values.steps.push(Value::Null);
            continue;
        }
        let actual = response.body_json().unwrap();
        compare(&substitute(&want["body"], &values), &actual, &format!("{label}.body"))?;
        values.steps.push(actual);
    }
    Ok(())
}

#[test]
fn every_golden_case_replays() {
    let all = cases();
    let failures: Vec<String> = all
        .iter()
        .filter_map(|case| replay(case).err().map(|why| format!("{}: {why}", case["name"])))
        .collect();
    assert!(failures.is_empty(), "cas en échec :\n{}", failures.join("\n"));
    println!("{} cas rejoués", all.len());
    assert_eq!(all.len(), 49);
}

#[test]
fn comparator_accepts_inclusion_and_generics_and_rejects_differences() {
    let actual = json!({"id": "00000000-0000-4000-8000-0000000000a1", "when": "2026-11-08T00:00:00.000Z", "n": 3,
        "name": "default.abc", "list": [1, 2], "extra": true});
    let wanted = json!({"id": "{{uuid}}", "when": "{{iso-date}}", "n": "{{number}}", "name": "default.{{string}}", "list": [1, 2]});
    assert!(compare(&wanted, &actual, "$").is_ok());
    for bad in [
        json!({"id": "x"}),
        json!({"list": [1]}),
        json!({"missing": 1}),
        json!({"name": "other.{{string}}"}),
        json!({"n": "{{uuid}}"}),
    ] {
        assert!(compare(&bad, &actual, "$").is_err(), "{bad}");
    }
}
