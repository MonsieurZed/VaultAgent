//! `unlock` et `lock` de bout en bout contre un faux serveur, sans attente réelle.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::rc::Rc;

use common::{Outcome, TempDir, execute};
use serde_json::Value;
use vaultagent_client::fake_transport::FakeTransport;

const PENDING: &str =
    "{\"vaultId\":\"default\",\"requestId\":\"r1\",\"state\":\"pending\",\"expiresUtc\":\"2026-11-08T00:05:00Z\"}";
const STILL_PENDING: &str = "{\"unlocked\":false,\"unlockRequest\":{\"requestId\":\"r1\",\"state\":\"pending\"}}";

/// Exécute une commande avec une configuration valide.
fn run(fake: FakeTransport, args: &[&str]) -> (Outcome, Rc<FakeTransport>) {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(fake);
    let mut all: Vec<&str> = args.to_vec();
    all.extend(["--config", config.to_str().unwrap()]);
    let outcome = execute(&fake, &all);
    (outcome, fake)
}

/// Sortie JSON d'un résultat.
fn out(outcome: &Outcome) -> Value {
    serde_json::from_str(outcome.out.trim()).unwrap()
}

/// Chemins appelés, dans l'ordre.
fn paths(fake: &FakeTransport) -> Vec<String> {
    fake.calls().into_iter().map(|call| call.path).collect()
}

#[test]
fn already_unlocked_answers_immediately() {
    let (outcome, fake) = run(
        FakeTransport::new().reply(200, "{\"state\":\"unlocked\",\"unlocked\":true}"),
        &["unlock", "--wait", "30"],
    );
    assert_eq!(outcome.exit.code(), 0);
    assert_eq!(out(&outcome)["unlocked"], true);
    assert_eq!(paths(&fake), ["/api/agent/request-unlock"]);
    assert_eq!(outcome.pauses, 0);
}

#[test]
fn without_wait_the_pending_request_is_relayed_and_exit_is_four() {
    let (outcome, fake) = run(FakeTransport::new().reply(200, PENDING), &["--vault", "work", "unlock"]);
    assert_eq!(outcome.exit.code(), 4);
    assert_eq!(out(&outcome)["requestId"], "r1");
    assert_eq!(out(&outcome)["state"], "pending");
    assert_eq!(fake.calls().len(), 1);
    assert_eq!(fake.calls()[0].body, "{\"vaultId\":\"work\"}");
    assert_eq!(outcome.pauses, 0);
}

#[test]
fn approved_during_the_wait_is_unlocked() {
    let fake = FakeTransport::new()
        .reply(200, PENDING)
        .reply(200, STILL_PENDING)
        .reply(200, "{\"unlocked\":true,\"unlockRequest\":null}");
    let (outcome, fake) = run(fake, &["unlock", "--wait", "30"]);
    assert_eq!(outcome.exit.code(), 0);
    let body = out(&outcome);
    assert_eq!(
        (
            body["requestId"].as_str(),
            body["state"].as_str(),
            body["unlocked"].as_bool()
        ),
        (Some("r1"), Some("unlocked"), Some(true))
    );
    assert_eq!(
        paths(&fake),
        ["/api/agent/request-unlock", "/api/agent/status", "/api/agent/status"]
    );
    assert_eq!(outcome.pauses, 2);
}

#[test]
fn denied_and_failed_requests_exit_two_without_a_new_request() {
    for state in ["denied", "failed"] {
        let status = format!("{{\"unlocked\":false,\"unlockRequest\":{{\"requestId\":\"r1\",\"state\":\"{state}\"}}}}");
        let (outcome, fake) = run(
            FakeTransport::new().reply(200, PENDING).reply(200, &status),
            &["unlock", "--wait", "30"],
        );
        assert_eq!(outcome.exit.code(), 2, "{state}");
        assert_eq!(out(&outcome)["state"], state);
        assert_eq!(out(&outcome)["unlocked"], false);
        assert_eq!(fake.calls().len(), 2);
    }
}

#[test]
fn vanished_request_is_expired_and_exit_is_four() {
    let (outcome, fake) = run(
        FakeTransport::new()
            .reply(200, PENDING)
            .reply(200, "{\"unlocked\":false,\"unlockRequest\":null}"),
        &["unlock", "--wait", "30"],
    );
    assert_eq!(outcome.exit.code(), 4);
    assert_eq!(out(&outcome)["state"], "expired");
    assert_eq!(out(&outcome)["requestId"], "r1");
    assert_eq!(fake.calls().len(), 2);
}

#[test]
fn deadline_reached_gives_waiting_after_the_expected_polls() {
    let mut fake = FakeTransport::new().reply(200, PENDING);
    for _ in 0..3 {
        fake = fake.reply(200, STILL_PENDING);
    }
    let (outcome, fake) = run(fake, &["unlock", "--wait", "5"]);
    assert_eq!(outcome.exit.code(), 4);
    assert_eq!(out(&outcome)["state"], "waiting");
    assert_eq!(out(&outcome)["unlocked"], false);
    assert_eq!(out(&outcome)["requestId"], "r1");
    assert_eq!(outcome.pauses, 3);
    assert_eq!(fake.calls().len(), 4);
}

#[test]
fn out_of_range_wait_is_a_usage_error_without_network() {
    for wait in ["-1", "301", "100000"] {
        let (outcome, fake) = run(FakeTransport::new(), &["unlock", "--wait", wait]);
        assert_eq!(outcome.exit.code(), 1, "{wait}");
        assert_eq!(fake.calls().len(), 0);
    }
    let (edge, _) = run(FakeTransport::new().reply(200, PENDING), &["unlock", "--wait", "0"]);
    assert_eq!(edge.exit.code(), 4);
}

#[test]
fn lock_relays_the_server_answer() {
    let (outcome, fake) = run(FakeTransport::new().reply(200, "{\"locked\":true}"), &["lock"]);
    assert_eq!(outcome.exit.code(), 0);
    assert_eq!(outcome.out, "{\"locked\":true}\n");
    assert_eq!(fake.calls()[0].path, "/api/agent/lock");
    assert_eq!(fake.calls()[0].body, "{}");
}
