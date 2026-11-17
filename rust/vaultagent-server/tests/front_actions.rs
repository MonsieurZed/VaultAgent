//! Formulaires `POST /admin/<action>` : contrôles d'origine et de propriétaire, redirections 303 et codes de message.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::front::{form, form_as, html, location};
use common::service::{Client, ORIGIN, OWNER, Service, VaultState};
use common::{FICTITIOUS, connectable};
use serde_json::{Value, json};
use vaultagent_engine::stage_error;
use vaultagent_server::Request;

const FORM: &str = "application/x-www-form-urlencoded";
const ACTIONS: [&str; 8] = [
    "configure",
    "unlock",
    "lock",
    "pairing",
    "pair-decide",
    "decide-unlock",
    "auto-unlock",
    "revoke",
];

/// Code d'erreur JSON d'une réponse.
fn error_of(service: &Service, request: &Request) -> (u16, String) {
    let response = service.send(request);
    let body = response.body_json().unwrap_or(Value::Null);
    (response.status, body["error"].as_str().unwrap_or_default().to_owned())
}

/// État JSON administrateur.
fn state(service: &Service) -> Value {
    let request = common::front::get("/api/admin/state");
    service.send(&request).body_json().unwrap()
}

/// Ouvre une demande d'association et rend son identifiant.
fn pair_request(service: &Service) -> String {
    let request = Request::new("POST", "/api/pair", "127.0.0.1")
        .with_header("content-type", "application/json")
        .with_body(
            json!({"action": "request", "name": "Portable factice"})
                .to_string()
                .as_bytes(),
        );
    service.send(&request).body_json().unwrap()["requestId"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn every_action_refuses_a_missing_or_foreign_origin() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    for action in ACTIONS {
        for origin in [None, Some("https://evil.example.test")] {
            let request = form_as(action, "name=x", origin, Some(OWNER), FORM);
            let response = service.send(&request);
            assert_eq!(
                (response.status, location(&response).as_str()),
                (303, "/?msg=stale-page"),
                "{action}"
            );
        }
    }
    assert_eq!(state(&service)["unlocked"], true);
}

#[test]
fn every_action_requires_the_owner() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    for action in ACTIONS {
        for owner in [None, Some("intrus")] {
            let request = form_as(action, "name=x", Some(ORIGIN), owner, FORM);
            assert_eq!(
                error_of(&service, &request),
                (403, "owner-required".to_owned()),
                "{action}"
            );
        }
    }
}

#[test]
fn the_body_type_and_the_action_name_are_checked() {
    let service = Service::new(VaultState::Unlocked, Client::None);
    let json_body = form_as("lock", "{}", Some(ORIGIN), Some(OWNER), "application/json");
    assert_eq!(error_of(&service, &json_body), (400, "request-refused".to_owned()));
    assert_eq!(location(&service.send(&form("write", ""))), "/?msg=unknown-action");
    let big = form("lock", &format!("x={}", "a".repeat(30_000)));
    assert_eq!(location(&service.send(&big)), "/?msg=too-large");
    assert_eq!(state(&service)["unlocked"], true);
}

#[test]
fn configure_saves_then_redirects_without_the_password() {
    let service = Service::new(VaultState::Unconfigured, Client::None);
    connectable(&service.factory);
    let body = format!(
        "server=https%3A%2F%2Fvault.example.test%2F&email=probe%40example.invalid&password={FICTITIOUS}&code=&automatic=on&minutes=20"
    );
    let response = service.send(&form("configure", &body));
    assert_eq!(
        (response.status, location(&response).as_str()),
        (303, "/?view=vault&msg=saved")
    );
    assert!(response.body.is_empty() && !location(&response).contains(FICTITIOUS));
    let after = state(&service);
    assert_eq!(
        (after["configured"].clone(), after["unlocked"].clone()),
        (json!(true), json!(true))
    );
    assert_eq!(after["timeoutMinutes"], 20);
}

#[test]
fn configure_failures_give_their_message_codes() {
    let service = Service::new(VaultState::Unconfigured, Client::None);
    let bad = "server=http%3A%2F%2Fvault.example.test&email=probe%40example.invalid&password=x&automatic=on&minutes=15";
    assert_eq!(
        location(&service.send(&form("configure", bad))),
        "/?view=vault&msg=invalid-input"
    );
    assert_eq!(state(&service)["configured"], false);
}

#[test]
fn a_refused_connection_gives_the_login_message() {
    let service = Service::new(VaultState::Locked, Client::None);
    service.factory.engine().push_error(stage_error("login", "credentials"));
    let refused = service.send(&form("unlock", "code="));
    assert_eq!(location(&refused), "/?msg=login-refused");
    assert_eq!(state(&service)["unlocked"], false);
}

#[test]
fn unlock_and_lock_redirect_to_the_form_page() {
    let service = Service::new(VaultState::Locked, Client::None);
    connectable(&service.factory);
    let opened = service.send(&form("unlock", "code=&view=help"));
    assert_eq!(
        (opened.status, location(&opened).as_str()),
        (303, "/?view=help&msg=saved")
    );
    assert_eq!(state(&service)["unlocked"], true);
    let closed = service.send(&form("lock", "view=vault"));
    assert_eq!(location(&closed), "/?view=vault&msg=saved");
    assert_eq!(state(&service)["unlocked"], false);
    let fallback = service.send(&form("lock", "view=%3Cscript%3E"));
    assert_eq!(location(&fallback), "/?msg=saved");
    let removed = service.send(&form("lock", "view=add"));
    assert_eq!(location(&removed), "/?msg=saved");
}

#[test]
fn pairing_answers_a_no_store_page_and_never_a_redirect() {
    let service = Service::new(VaultState::Locked, Client::None);
    let response = service.send(&form("pairing", "name=Poste+de+travail"));
    assert_eq!(response.status, 200);
    assert_eq!(response.header("cache-control"), Some("no-store"));
    assert!(response.header("location").is_none());
    let body = html(&response);
    let code = body
        .split("<code class=\"pair-code\">")
        .nth(1)
        .unwrap()
        .split('<')
        .next()
        .unwrap()
        .to_owned();
    assert!(code.starts_with("default."));
    assert!(body.contains("<details><summary>Afficher le code</summary>") && body.contains("<a href=\"/\">Retour</a>"));
    assert!(body.contains("09:05") && !body.contains("<script"));
    for (_, value) in &response.headers {
        assert!(!value.contains(&code));
    }
    let paired = Request::new("POST", "/api/pair", "127.0.0.1")
        .with_header("content-type", "application/json")
        .with_body(json!({"code": code}).to_string().as_bytes());
    assert_eq!(service.send(&paired).status, 200);
}

#[test]
fn a_refused_pairing_redirects_with_a_code() {
    let service = Service::new(VaultState::Unconfigured, Client::None);
    assert_eq!(
        location(&service.send(&form("pairing", "name=Poste"))),
        "/?msg=not-configured"
    );
    let blank = Service::new(VaultState::Locked, Client::None);
    assert_eq!(location(&blank.send(&form("pairing", "name="))), "/?msg=invalid-input");
}

#[test]
fn pair_decisions_save_or_expire() {
    let service = Service::new(VaultState::Locked, Client::None);
    let id = pair_request(&service);
    let approved = service.send(&form("pair-decide", &format!("requestId={id}&approve=true")));
    assert_eq!(location(&approved), "/?msg=saved");
    assert_eq!(state(&service)["devices"].as_array().map(Vec::len), Some(1));
    let gone = service.send(&form("pair-decide", &format!("requestId={id}&approve=true")));
    assert_eq!(location(&gone), "/?msg=expired");
    let other = pair_request(&service);
    let denied = service.send(&form("pair-decide", &format!("requestId={other}&approve=false")));
    assert_eq!(location(&denied), "/?msg=saved");
    assert_eq!(state(&service)["pairRequests"].as_array().map(Vec::len), Some(0));
}

#[test]
fn unlock_decisions_use_the_clicked_button() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let (_, asked) = service.call("request-unlock", &json!({}));
    let id = asked["requestId"].as_str().unwrap();
    connectable(&service.factory);
    let approved = service.send(&form(
        "decide-unlock",
        &format!("requestId={id}&approve=true&code=&view=help"),
    ));
    assert_eq!(location(&approved), "/?view=help&msg=saved");
    assert_eq!(state(&service)["unlocked"], true);
    let gone = service.send(&form("decide-unlock", "requestId=inconnue&approve=true"));
    assert_eq!(location(&gone), "/?msg=expired");
}

#[test]
fn auto_unlock_and_revoke_change_the_device_list() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let device = state(&service)["devices"][0]["id"].as_str().unwrap().to_owned();
    let enabled = service.send(&form(
        "auto-unlock",
        &format!("deviceId={device}&enabled=false&enabled=true"),
    ));
    assert_eq!(location(&enabled), "/?msg=saved");
    assert_eq!(state(&service)["devices"][0]["autoUnlock"], true);
    let disabled = service.send(&form("auto-unlock", &format!("deviceId={device}&enabled=false")));
    assert_eq!(location(&disabled), "/?msg=saved");
    assert_eq!(state(&service)["devices"][0]["autoUnlock"], false);
    let unknown = service.send(&form("auto-unlock", "deviceId=absent&enabled=true"));
    assert_eq!(location(&unknown), "/?msg=invalid-input");
    let revoked = service.send(&form("revoke", &format!("deviceId={device}")));
    assert_eq!(location(&revoked), "/?msg=saved");
    assert_eq!(state(&service)["devices"].as_array().map(Vec::len), Some(0));
}
