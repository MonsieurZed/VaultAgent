//! Codes de sortie 0, 1, 2, 4 et 5 par la bibliothèque, et forme des sorties : un JSON sur stdout, une ligne versionnée sur stderr.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::rc::Rc;

use common::{Outcome, TempDir, execute};
use serde_json::Value;
use vaultagent_client::fake_transport::FakeTransport;
use vaultagent_client::transport::TransportError;

/// Exécute une commande avec une configuration valide.
fn run(fake: FakeTransport, args: &[&str]) -> Outcome {
    let dir = TempDir::new();
    let config = dir.write_config();
    let mut all: Vec<&str> = args.to_vec();
    all.extend(["--config", config.to_str().unwrap()]);
    execute(&Rc::new(fake), &all)
}

/// Vérifie qu'une sortie en échec n'écrit rien sur stdout et une ligne versionnée sur stderr.
fn assert_error_line(outcome: &Outcome, code: &str) {
    assert_eq!(outcome.out, "");
    assert_eq!(outcome.err.lines().count(), 1);
    let line: Value = serde_json::from_str(outcome.err.trim()).unwrap();
    assert_eq!(line["version"], 1);
    assert_eq!(line["error"], code);
}

#[test]
fn exit_zero_prints_only_the_server_json() {
    let outcome = run(FakeTransport::new().reply(200, "{\"unlocked\":true}"), &["status"]);
    assert_eq!(outcome.exit.code(), 0);
    assert_eq!(outcome.out, "{\"unlocked\":true}\n");
    assert_eq!(outcome.err, "");
}

#[test]
fn exit_one_for_usage() {
    let outcome = run(FakeTransport::new(), &["preview", "not-a-uuid"]);
    assert_eq!(outcome.exit.code(), 1);
    assert_error_line(&outcome, "usage");
}

#[test]
fn exit_two_for_a_server_refusal() {
    let outcome = run(
        FakeTransport::new().reply(403, "{\"error\":\"unauthorized\"}"),
        &["status"],
    );
    assert_eq!(outcome.exit.code(), 2);
    assert_error_line(&outcome, "unauthorized");
}

#[test]
fn exit_four_when_the_vault_is_locked() {
    let outcome = run(FakeTransport::new().reply(423, "{\"error\":\"locked\"}"), &["catalog"]);
    assert_eq!(outcome.exit.code(), 4);
    assert_error_line(&outcome, "locked");
}

#[test]
fn exit_five_when_the_outcome_is_unknown() {
    let outcome = run(FakeTransport::new().fail(TransportError::Indeterminate), &["lock"]);
    assert_eq!(outcome.exit.code(), 5);
    assert_error_line(&outcome, "indeterminate");
}
