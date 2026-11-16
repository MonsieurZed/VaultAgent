//! Contrôles communs du cœur HTTP : ordre des refus, corps JSON, taille, sélection du coffre.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::service::{Client, ORIGIN, OWNER, Service, VaultState};
use serde_json::{Value, json};
use vaultagent_server::Request;

/// Aide de test : service.
fn service() -> Service {
    Service::new(VaultState::Unlocked, Client::Paired)
}

/// Aide de test : error of.
fn error_of(service: &Service, request: &Request) -> (u16, String) {
    let response = service.send(request);
    let body = response.body_json().unwrap();
    (response.status, body["error"].as_str().unwrap_or_default().to_owned())
}

#[test]
fn healthz_needs_neither_proxy_nor_token() {
    let response = service().send(&Request::new("GET", "/healthz?x=1", "10.9.9.9"));
    assert_eq!(
        (response.status, response.body_json()),
        (200, Some(json!({"alive": true})))
    );
    assert_eq!(response.header("cache-control"), Some("no-store"));
    assert_eq!(response.header("x-content-type-options"), Some("nosniff"));
}

#[test]
fn proxy_is_checked_before_origin_and_origin_before_authentication() {
    let service = service();
    let stranger = Request::new("POST", "/api/agent/status", "10.9.9.9").with_header("origin", "https://evil.test");
    assert_eq!(error_of(&service, &stranger), (403, "proxy-required".to_owned()));
    let foreign = Request::new("POST", "/api/agent/status", "127.0.0.1").with_header("origin", "https://evil.test");
    assert_eq!(error_of(&service, &foreign), (403, "origin-refused".to_owned()));
    let own = Request::new("POST", "/api/agent/status", "127.0.0.1").with_header("origin", ORIGIN);
    assert_eq!(error_of(&service, &own), (403, "unauthorized".to_owned()));
}

#[test]
fn agent_routes_check_token_then_method_then_content_type() {
    let service = service();
    let get = Request::new("GET", "/api/agent/status", "127.0.0.1");
    assert_eq!(error_of(&service, &get), (403, "unauthorized".to_owned()));
    let token = format!("Bearer {}", service.token.clone().unwrap());
    let get = get.with_header("authorization", &token);
    assert_eq!(error_of(&service, &get), (400, "method-refused".to_owned()));
    let plain = Request::new("POST", "/api/agent/status", "127.0.0.1")
        .with_header("authorization", &token)
        .with_header("content-type", "application/jsonx")
        .with_body(b"{}");
    assert_eq!(error_of(&service, &plain), (400, "json-required".to_owned()));
    let loud = Request::new("POST", "/api/agent/status", "127.0.0.1")
        .with_header("authorization", &token)
        .with_header("content-type", "Application/JSON; charset=utf-8")
        .with_body(b"{}");
    assert_eq!(service.send(&loud).status, 200);
}

#[test]
fn body_limit_is_24576_bytes() {
    let service = service();
    let sized = |length: usize| {
        let filler = "x".repeat(length - "{\"padding\":\"\"}".len());
        json!({"padding": filler}).to_string()
    };
    let at_limit = sized(24_576);
    assert_eq!(at_limit.len(), 24_576);
    let ok = service.request("status", &Value::Null).with_body(at_limit.as_bytes());
    assert_eq!(service.send(&ok).status, 200);
    let over = service
        .request("status", &Value::Null)
        .with_body(sized(24_577).as_bytes());
    assert_eq!(error_of(&service, &over), (400, "request-size".to_owned()));
}

#[test]
fn unknown_route_null_body_and_unknown_vault_are_refused() {
    let service = service();
    assert_eq!(
        service.call("nope", &json!({})),
        (400, json!({"error": "route-refused"}))
    );
    assert_eq!(
        service.call("status", &Value::Null),
        (400, json!({"error": "request-refused"}))
    );
    assert_eq!(
        service.call("status", &json!({"vaultId": "autre"})),
        (403, json!({"error": "unauthorized"}))
    );
    assert_eq!(
        service.call("status", &json!({"vaultId": 7})),
        (403, json!({"error": "unauthorized"}))
    );
}

#[test]
fn other_paths_need_the_owner_then_a_post_with_the_origin() {
    let service = service();
    let anonymous = Request::new("GET", "/api/admin/lock", "127.0.0.1");
    assert_eq!(error_of(&service, &anonymous), (403, "owner-required".to_owned()));
    let owner = anonymous.with_header("x-authentik-username", OWNER);
    assert_eq!(error_of(&service, &owner), (403, "origin-refused".to_owned()));
}

#[test]
fn status_never_carries_a_secret_or_a_version_key_of_the_rust_json_layer() {
    let service = service();
    let (status, body) = service.call("status", &json!({}));
    assert_eq!(status, 200);
    assert_eq!(body["version"], "3.0.0-preview.7");
    assert!(body["expiresUtc"].is_string() && body["timeoutMinutes"] == 15);
}
