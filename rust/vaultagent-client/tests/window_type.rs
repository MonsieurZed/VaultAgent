//! Commande `type` contre une fausse fenêtre et un faux serveur qui scelle une sentinelle.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use common::fake_windows::{FakeWindows, bank, json, target, window};
use serde_json::Value;
use vaultagent_client::config::ClientConfig;
use vaultagent_client::error::ClientError;
use vaultagent_client::exit_code::ExitCode;
use vaultagent_client::fake_transport::FakeTransport;
use vaultagent_client::server_address::ServerAddress;
use vaultagent_client::token::Token;
use vaultagent_client::transport::{Response, Transport, TransportError};
use vaultagent_client::unlock::Outcome;
use vaultagent_client::window_args::TypeArgs;
use vaultagent_client::window_commands::type_text;
use vaultagent_core::{SystemEntropy, seal};

const SENTINEL: &str = "SENTINEL-not-a-real-secret-4242";
const ITEM: &str = "0000000a-0000-4000-8000-00000000000c";
const ORIGIN: &str = "https://example.test";

/// Faux serveur qui scelle la sentinelle pour la clé reçue et compte les requêtes.
struct Sealing {
    requests: Rc<Cell<usize>>,
}

impl Transport for Sealing {
    /// Lit `publicKey`, scelle la sentinelle et rend l'enveloppe.
    fn post(&self, _: &str, _: Option<&str>, body: &[u8], _: Duration) -> Result<Response, TransportError> {
        self.requests.set(self.requests.get() + 1);
        let value: Value = serde_json::from_slice(body).unwrap();
        let envelope = seal(
            SENTINEL.as_bytes(),
            value["publicKey"].as_str().unwrap(),
            &mut SystemEntropy,
        )
        .unwrap();
        Ok(Response {
            status: 200,
            body: serde_json::to_vec(&envelope).unwrap(),
        })
    }
}

/// Configuration factice complète.
fn config() -> ClientConfig {
    ClientConfig {
        server: ServerAddress::parse("https://vault.example.test").unwrap(),
        token: Token::parse(&common::token()).unwrap(),
    }
}

/// Arguments de `type` vers l'origine factice.
fn type_args(field: &str, process: &str, title: &str) -> TypeArgs {
    TypeArgs {
        item_id: ITEM.to_owned(),
        field: field.to_owned(),
        origin: ORIGIN.to_owned(),
        window: target(process, title),
    }
}

/// Exécute `type` ; rend le résultat et le nombre de requêtes reçues par le faux serveur.
fn run_type(system: &FakeWindows, args: &TypeArgs) -> (Result<Outcome, ClientError>, usize) {
    let requests = Rc::new(Cell::new(0));
    let transport = Sealing {
        requests: Rc::clone(&requests),
    };
    let result = type_text(system, &transport, &config(), None, args);
    (result, requests.get())
}

#[test]
fn typing_into_the_matching_window_succeeds_without_printing_the_value() {
    let system = FakeWindows::new(vec![Some(bank())], vec![bank()]);
    let (result, requests) = run_type(&system, &type_args("password", "chrome.exe", "Ma banque"));
    let outcome = result.unwrap();
    assert_eq!(outcome.exit, ExitCode::Success);
    assert_eq!(*system.typed.borrow(), SENTINEL);
    assert_eq!(requests, 1);
    assert!(!outcome.line.contains(SENTINEL));
    let report = json(&outcome.line);
    assert_eq!(report["typed"], true);
    assert_eq!(report["field"], "password");
    assert_eq!(report["clipboardUsed"], false);
    assert_eq!(report["target"]["process"], "chrome");
    assert!(report.get("version").is_none());
}

#[test]
fn a_foreground_that_does_not_match_is_refused_before_any_request() {
    let notepad = window(2, "notepad", "Ma banque");
    let system = FakeWindows::new(vec![Some(notepad)], vec![]);
    let (result, requests) = run_type(&system, &type_args("password", "chrome", "Ma banque"));
    let outcome = result.unwrap();
    assert_eq!(outcome.exit, ExitCode::TargetRefused);
    assert_eq!(requests, 0);
    assert_eq!(*system.typed.borrow(), "");
    let report = json(&outcome.line);
    assert_eq!(report["typed"], false);
    assert!(report["reason"].as_str().unwrap().contains("does not match"));
    assert_eq!(report["foreground"]["process"], "notepad");
    assert_eq!(report["expected"]["process"], "chrome");
    assert_eq!(report["expected"]["title"], "Ma banque");
}

#[test]
fn an_unreadable_foreground_is_refused_with_a_null_window() {
    let system = FakeWindows::new(vec![None], vec![]);
    let (result, requests) = run_type(&system, &type_args("totp", "chrome", "Ma banque"));
    let outcome = result.unwrap();
    assert_eq!(outcome.exit, ExitCode::TargetRefused);
    assert_eq!(requests, 0);
    assert!(json(&outcome.line)["foreground"].is_null());
}

#[test]
fn typing_stops_when_the_foreground_changes() {
    let other = window(3, "notepad", "Sans titre");
    let script = vec![Some(bank()), Some(bank()), Some(bank()), Some(bank()), Some(other)];
    let system = FakeWindows::new(script, vec![]);
    let (result, requests) = run_type(&system, &type_args("password", "chrome", "Ma banque"));
    let outcome = result.unwrap();
    assert_eq!(outcome.exit, ExitCode::TargetRefused);
    assert_eq!(requests, 1);
    assert_eq!(*system.typed.borrow(), &SENTINEL[..3]);
    assert!(!outcome.line.contains(SENTINEL));
    let report = json(&outcome.line);
    assert_eq!(report["typed"], false);
    assert!(report["reason"].as_str().unwrap().contains("changed"));
    assert_eq!(report["foreground"]["process"], "notepad");
}

#[test]
fn a_title_drift_during_typing_stops_it() {
    let drifted = window(1, "chrome", "Autre site");
    let system = FakeWindows::new(vec![Some(bank()), Some(bank()), Some(drifted)], vec![]);
    let (result, _) = run_type(&system, &type_args("password", "chrome", "Ma banque"));
    let outcome = result.unwrap();
    assert_eq!(outcome.exit, ExitCode::TargetRefused);
    assert_eq!(*system.typed.borrow(), &SENTINEL[..1]);
    assert!(
        json(&outcome.line)["reason"]
            .as_str()
            .unwrap()
            .contains("does not match")
    );
}

#[test]
fn invalid_arguments_fail_before_any_window_or_server_access() {
    let system = FakeWindows::new(vec![Some(bank())], vec![]);
    for args in [
        type_args("notes", "chrome", "Ma banque"),
        type_args("password", "chrome", "Google Chrome"),
        type_args("password", "chrome", "ab"),
        type_args("password", "a b", "Ma banque"),
    ] {
        let (result, requests) = run_type(&system, &args);
        assert!(matches!(result, Err(ClientError::Usage(_))));
        assert_eq!(requests, 0);
    }
    assert_eq!(system.reads.get(), 0);
}

#[test]
fn a_server_refusal_is_an_error_not_a_target_report() {
    let system = FakeWindows::new(vec![Some(bank())], vec![]);
    let fake = FakeTransport::new().reply(403, "{\"error\":\"origin-refused\"}");
    let args = type_args("password", "chrome", "Ma banque");
    let result = type_text(&system, &fake, &config(), None, &args);
    assert!(matches!(result, Err(ClientError::Server { status: 403, .. })));
    assert_eq!(*system.typed.borrow(), "");
}
