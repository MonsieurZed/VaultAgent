//! Erreurs lisibles du front : redirection des formulaires sans action, page d'erreur HTML d'un navigateur, JSON
//! inchangé pour les appelants sans en-tête de navigateur.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::front::{form, form_as, get, html, location};
use common::service::{Client, ORIGIN, OWNER, Service, VaultState};
use serde_json::Value;

const FORM: &str = "application/x-www-form-urlencoded";

#[test]
fn a_refused_origin_on_a_form_redirects_and_runs_nothing() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let request = form_as("lock", "view=add", Some("https://evil.example.test"), Some(OWNER), FORM);
    let response = service.send(&request);
    assert_eq!(
        (response.status, location(&response).as_str()),
        (303, "/?msg=stale-page")
    );
    let state = service.send(&get("/api/admin/state")).body_json().unwrap();
    assert_eq!(state["unlocked"], true);
}

#[test]
fn a_browser_form_with_another_body_type_is_redirected() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let request =
        form_as("lock", "{}", Some(ORIGIN), Some(OWNER), "application/json").with_header("sec-fetch-mode", "navigate");
    assert_eq!(location(&service.send(&request)), "/?msg=bad-request");
    let plain = form_as("lock", "{}", Some(ORIGIN), Some(OWNER), "application/json");
    assert_eq!(service.send(&plain).status, 400);
}

#[test]
fn the_redirect_message_is_shown_in_the_page() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let shown = html(&service.send(&get("/?msg=stale-page&lang=en")));
    assert!(shown.contains("The page was out of date, the action was not done. Try again."));
    let unknown = html(&service.send(&get("/?msg=nothing")));
    assert!(!unknown.contains("Action refusée par le serveur."));
    let refused = html(&service.send(&get("/?msg=refused")));
    assert!(refused.contains("Action refusée par le serveur."));
}

#[test]
fn a_browser_page_error_is_a_one_sentence_html_page() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let request = get("/inconnu").with_header("sec-fetch-mode", "navigate");
    let response = service.send(&request);
    assert_eq!(response.status, 403);
    assert_eq!(response.header("content-type"), Some("text/html; charset=utf-8"));
    assert!(
        response
            .header("content-security-policy")
            .unwrap()
            .starts_with("default-src 'none'")
    );
    assert_eq!(response.header("referrer-policy"), Some("same-origin"));
    let body = html(&response);
    assert!(body.contains("Page introuvable.") && body.contains("<a href=\"/\">Retour</a>"));
    assert!(!body.contains("<script"));
    let accepted = get("/inconnu").with_header("accept", "text/html,application/xhtml+xml");
    assert_eq!(
        service.send(&accepted).header("content-type"),
        Some("text/html; charset=utf-8")
    );
}

#[test]
fn the_same_request_without_browser_headers_keeps_the_json() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let response = service.send(&get("/inconnu"));
    assert_eq!(
        (response.status, response.body_json().unwrap()["error"].clone()),
        (403, Value::String("origin-refused".to_owned()))
    );
    assert_eq!(response.header("content-type"), Some("application/json"));
    let owner_less = service.send(&form_as("lock", "", Some(ORIGIN), None, FORM));
    assert_eq!(owner_less.body_json().unwrap()["error"], "owner-required");
    assert_eq!(
        location(&service.send(&form("lock", "view=help"))),
        "/?view=help&msg=saved"
    );
}
