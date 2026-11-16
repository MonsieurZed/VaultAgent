//! Routes d'administration du propriétaire et association : appareils, codes, demandes, session, écritures.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::service::{Client, ORIGIN, OWNER, Service, VaultState};
use common::{EMAIL, FICTITIOUS, SERVER, connectable};
use serde_json::{Value, json};
use vaultagent_engine::Reply;
use vaultagent_server::Request;

/// Requête du propriétaire avec corps JSON.
fn owner(method: &str, path: &str, body: &Value) -> Request {
    Request::new(method, path, "127.0.0.1")
        .with_header("x-authentik-username", OWNER)
        .with_header("origin", ORIGIN)
        .with_header("content-type", "application/json")
        .with_body(body.to_string().as_bytes())
}

/// Appelle une route d'administration `POST`.
fn admin(service: &Service, name: &str, body: &Value) -> (u16, Value) {
    let response = service.send(&owner("POST", &format!("/api/admin/{name}"), body));
    (response.status, response.body_json().unwrap())
}

/// Lit l'état administrateur d'un coffre.
fn state(service: &Service, query: &str) -> (u16, Value) {
    let request = Request::new("GET", &format!("/api/admin/state{query}"), "127.0.0.1")
        .with_header("x-authentik-username", OWNER);
    let response = service.send(&request);
    (response.status, response.body_json().unwrap())
}

/// Appelle `POST /api/pair`, sans authentification.
fn pair(service: &Service, body: &Value) -> (u16, Value) {
    let request = Request::new("POST", "/api/pair", "127.0.0.1")
        .with_header("content-type", "application/json")
        .with_body(body.to_string().as_bytes());
    let response = service.send(&request);
    (response.status, response.body_json().unwrap())
}

/// Appelle la route agent `vaults` avec un jeton et rend le statut.
fn agent_status(service: &Service, token: &str) -> u16 {
    let request = Request::new("POST", "/api/agent/vaults", "127.0.0.1")
        .with_header("content-type", "application/json")
        .with_header("authorization", &format!("Bearer {token}"))
        .with_body(b"{}");
    service.send(&request).status
}

/// Statut et code d'erreur d'une réponse JSON.
fn code_of((status, body): &(u16, Value)) -> (u16, String) {
    (*status, body["error"].as_str().unwrap_or_default().to_owned())
}

#[test]
fn admin_code_pairs_one_client_once_and_the_token_works() {
    let service = Service::new(VaultState::Unlocked, Client::None);
    let (status, issued) = admin(&service, "pairing", &json!({"name": "Poste factice"}));
    assert_eq!(status, 200);
    let code = issued["code"].as_str().unwrap().to_owned();
    let (status, paired) = pair(&service, &json!({"code": code}));
    assert_eq!((status, paired["vaultId"].as_str()), (200, Some("default")));
    assert_eq!(agent_status(&service, paired["token"].as_str().unwrap()), 200);
    assert_eq!(
        code_of(&pair(&service, &json!({"code": code}))),
        (400, "pairing-refused".to_owned())
    );
    let devices = state(&service, "").1["devices"].clone();
    assert_eq!(devices[0]["name"], "Poste factice");
}

#[test]
fn pairing_needs_an_account_and_a_name() {
    let service = Service::new(VaultState::Unconfigured, Client::None);
    assert_eq!(
        code_of(&admin(&service, "pairing", &json!({"name": "x"}))),
        (400, "not-configured".to_owned())
    );
    let service = Service::new(VaultState::Unlocked, Client::None);
    assert_eq!(
        code_of(&admin(&service, "pairing", &json!({"name": 5}))),
        (400, "invalid-device".to_owned())
    );
}

#[test]
fn pair_request_is_listed_approved_and_polled_once() {
    let service = Service::new(VaultState::Locked, Client::None);
    let (_, opened) = pair(&service, &json!({"action": "request", "name": "Portable factice"}));
    let (id, secret) = (
        opened["requestId"].as_str().unwrap(),
        opened["secret"].as_str().unwrap(),
    );
    let listed = state(&service, "").1["pairRequests"].clone();
    assert_eq!(listed[0]["verification"], opened["verification"]);
    let decision = json!({"requestId": id, "approve": true});
    assert_eq!(admin(&service, "pair-decide", &decision), (200, json!({"saved": true})));
    let poll = json!({"action": "poll", "requestId": id, "secret": secret});
    let (status, approved) = pair(&service, &poll);
    assert_eq!((status, approved["state"].as_str()), (200, Some("approved")));
    assert_eq!(approved["vaultId"], "default");
    assert_eq!(code_of(&pair(&service, &poll)), (400, "pairing-refused".to_owned()));
}

#[test]
fn pair_request_can_be_denied_and_unknown_ones_expire() {
    let service = Service::new(VaultState::Locked, Client::None);
    let (_, opened) = pair(&service, &json!({"action": "request", "name": "Portable factice"}));
    let id = opened["requestId"].as_str().unwrap();
    assert_eq!(
        admin(&service, "pair-decide", &json!({"requestId": id, "approve": false})).0,
        200
    );
    let poll = json!({"action": "poll", "requestId": id, "secret": opened["secret"]});
    assert_eq!(pair(&service, &poll).1, json!({"state": "denied"}));
    let unknown = json!({"requestId": "inconnu", "approve": true});
    assert_eq!(
        code_of(&admin(&service, "pair-decide", &unknown)),
        (400, "request-expired".to_owned())
    );
    let bare = Service::new(VaultState::Unconfigured, Client::None);
    assert_eq!(
        code_of(&admin(&bare, "pair-decide", &unknown)),
        (400, "not-configured".to_owned())
    );
}

#[test]
fn pair_request_checks_the_name_before_the_key() {
    let service = Service::new(VaultState::Locked, Client::None);
    let blank = json!({"action": "request", "name": "", "approverPublicKey": 5});
    assert_eq!(code_of(&pair(&service, &blank)), (400, "invalid-device".to_owned()));
    let typed = json!({"action": "request", "name": "Poste", "approverPublicKey": 5});
    assert_eq!(code_of(&pair(&service, &typed)), (400, "invalid-key".to_owned()));
}

#[test]
fn auto_unlock_and_revoke_change_the_device_list() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let listed = state(&service, "").1;
    let device = listed["devices"][0]["id"].as_str().unwrap().to_owned();
    assert_eq!(listed["devices"][0]["autoUnlock"], false);
    let enable = json!({"deviceId": device, "enabled": true});
    assert_eq!(admin(&service, "auto-unlock", &enable), (200, json!({"saved": true})));
    assert_eq!(state(&service, "").1["devices"][0]["autoUnlock"], true);
    let loose = json!({"deviceId": device, "enabled": "oui"});
    assert_eq!(
        code_of(&admin(&service, "auto-unlock", &loose)),
        (400, "invalid-device".to_owned())
    );
    assert_eq!(
        admin(&service, "revoke", &json!({"deviceId": device})),
        (200, json!({"revoked": true}))
    );
    let after = state(&service, "").1;
    assert_eq!(
        (after["devices"].clone(), after["unlocked"].clone()),
        (json!([]), json!(false))
    );
}

#[test]
fn session_routes_configure_lock_and_unlock() {
    let service = Service::new(VaultState::Unconfigured, Client::None);
    let account = json!({"server": SERVER, "email": EMAIL, "password": FICTITIOUS, "timeoutMinutes": 15});
    let (status, configured) = admin(&service, "configure", &account);
    assert_eq!((status, configured["unlocked"].clone()), (200, json!(true)));
    let bad = json!({"server": "http://x.test", "email": EMAIL, "timeoutMinutes": 15});
    assert_eq!(
        code_of(&admin(&service, "configure", &bad)),
        (400, "invalid-account".to_owned())
    );
    assert_eq!(admin(&service, "lock", &json!({})), (200, json!({"locked": true})));
    connectable(&service.factory);
    let (status, opened) = admin(&service, "unlock", &json!({}));
    assert_eq!((status, opened["unlocked"].clone()), (200, json!(true)));
}

#[test]
fn owner_decides_a_client_unlock_request() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let (_, asked) = service.call("request-unlock", &json!({}));
    let id = asked["requestId"].as_str().unwrap();
    assert_eq!(state(&service, "").1["unlockRequests"][0]["requestId"], id);
    let loose = json!({"requestId": id, "approve": "oui"});
    assert_eq!(
        code_of(&admin(&service, "decide-unlock", &loose)),
        (400, "request-expired".to_owned())
    );
    connectable(&service.factory);
    let (status, decided) = admin(&service, "decide-unlock", &json!({"requestId": id, "approve": true}));
    assert_eq!((status, decided["state"].as_str()), (200, Some("approved")));
    assert_eq!(state(&service, "").1["unlocked"], true);
}

#[test]
fn vault_choice_and_unknown_routes_are_refused() {
    let service = Service::new(VaultState::Unlocked, Client::None);
    assert_eq!(
        code_of(&state(&service, "?vaultId=absent")),
        (400, "vault-missing".to_owned())
    );
    assert_eq!(state(&service, "?vaultId=").0, 200);
    assert_eq!(
        code_of(&admin(&service, "lock", &json!({"vaultId": 5}))),
        (400, "vault-missing".to_owned())
    );
    assert_eq!(
        code_of(&admin(&service, "inconnue", &json!({}))),
        (400, "route-refused".to_owned())
    );
    let plain = Request::new("POST", "/api/admin/lock", "127.0.0.1")
        .with_header("x-authentik-username", OWNER)
        .with_header("origin", ORIGIN)
        .with_header("content-type", "text/plain")
        .with_body(b"{}");
    let response = service.send(&plain);
    assert_eq!(response.status, 400);
    let get = Request::new("GET", "/api/admin/lock", "127.0.0.1").with_header("x-authentik-username", OWNER);
    assert_eq!(service.send(&get).body_json().unwrap()["error"], "origin-refused");
}

#[test]
fn owner_can_preview_writes_without_a_token() {
    let service = Service::new(VaultState::Unlocked, Client::None);
    service.script(vec![
        Reply::Unit,
        Reply::Folders(Vec::new()),
        Reply::Items(Vec::new()),
        Reply::Items(Vec::new()),
    ]);
    let (status, listing) = admin(&service, "write-preview", &json!({}));
    assert_eq!(status, 200);
    assert_eq!(listing, json!({"folders": [], "items": [], "trash": []}));
}
