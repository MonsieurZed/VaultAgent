//! Commandes de bout en bout contre un faux serveur : sorties, codes de sortie, association, secret absent.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::rc::Rc;

use common::{TempDir, execute, token};
use vaultagent_client::exit_code::ExitCode;
use vaultagent_client::fake_transport::FakeTransport;

const OPENED: &str =
    "{\"requestId\":\"r1\",\"secret\":\"fake-secret\",\"verification\":\"ABCD-EFGH\",\"expiresInSeconds\":30}";

#[test]
fn status_prints_the_server_body_on_one_line() {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(FakeTransport::new().reply(200, "{\"unlocked\":false}"));
    let outcome = execute(&fake, &["status", "--config", config.to_str().unwrap()]);
    assert_eq!(outcome.exit, ExitCode::Success);
    assert_eq!(outcome.out, "{\"unlocked\":false}\n");
    assert_eq!(outcome.err.len(), 0);
    assert_eq!(
        fake.calls()[0].bearer.as_deref(),
        Some(format!("Bearer {}", token()).as_str())
    );
}

#[test]
fn locked_vault_exits_four_with_a_json_error_line() {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(FakeTransport::new().reply(423, "{\"error\":\"locked\"}"));
    let outcome = execute(&fake, &["catalog", "--config", config.to_str().unwrap()]);
    assert_eq!(outcome.exit.code(), 4);
    assert_eq!(outcome.out.len(), 0);
    let line: serde_json::Value = serde_json::from_str(outcome.err.trim()).unwrap();
    assert_eq!(line["version"], 1);
    assert_eq!(line["error"], "locked");
}

#[test]
fn vault_required_is_a_refusal_with_its_code() {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(FakeTransport::new().reply(400, "{\"error\":\"vault-required\"}"));
    let outcome = execute(&fake, &["vaults", "--config", config.to_str().unwrap()]);
    assert_eq!(outcome.exit, ExitCode::Refused);
    assert!(outcome.err.contains("vault-required"));
}

#[test]
fn missing_configuration_is_a_usage_error_without_network() {
    let dir = TempDir::new();
    let missing = dir.path().join("absent.json");
    let fake = Rc::new(FakeTransport::new());
    let outcome = execute(&fake, &["status", "--config", missing.to_str().unwrap()]);
    assert_eq!(outcome.exit, ExitCode::Usage);
    assert_eq!(fake.calls().len(), 0);
}

#[test]
fn vault_option_is_sent_in_the_body() {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(FakeTransport::new().reply(200, "{}"));
    execute(
        &fake,
        &["status", "--vault", "work", "--config", config.to_str().unwrap()],
    );
    assert_eq!(fake.calls()[0].body, "{\"vaultId\":\"work\"}");
}

#[test]
fn token_never_reaches_stdout_or_stderr() {
    let dir = TempDir::new();
    let config = dir.write_config();
    let path = config.to_str().unwrap();
    let replies = [
        (200, "{}"),
        (423, "{\"error\":\"locked\"}"),
        (403, "{\"error\":\"unauthorized\"}"),
        (500, "boom"),
    ];
    for (status, body) in replies {
        let fake = Rc::new(FakeTransport::new().reply(status, body));
        let outcome = execute(&fake, &["status", "--config", path]);
        assert!(!outcome.out.contains(&token()) && !outcome.err.contains(&token()));
        assert!(!outcome.out.contains("va1.") && !outcome.err.contains("va1."));
    }
    let timed_out = Rc::new(FakeTransport::new().fail(vaultagent_client::transport::TransportError::Indeterminate));
    let outcome = execute(&timed_out, &["status", "--config", path]);
    assert_eq!(outcome.exit, ExitCode::Indeterminate);
    assert!(!outcome.err.contains(&token()));
}

#[test]
fn pair_request_saves_the_configuration_then_status_uses_it() {
    let dir = TempDir::new();
    let path = dir.path().join("vaultagent").join("client.json");
    let approved = {
        let token = token();
        format!("{{\"state\":\"approved\",\"token\":\"{token}\",\"deviceId\":\"d\",\"vaultId\":\"default\"}}")
    };
    let fake = Rc::new(
        FakeTransport::new()
            .reply(200, OPENED)
            .reply(200, "{\"state\":\"pending\",\"expiresInSeconds\":28}")
            .reply(200, &approved)
            .reply(200, "{\"unlocked\":true}"),
    );
    let config = path.to_str().unwrap();
    let paired = execute(
        &fake,
        &["pair-request", "https://vault.example.test", "--config", config],
    );
    assert_eq!(paired.exit, ExitCode::Success);
    assert_eq!(paired.out, "{\"paired\":true,\"vaultId\":\"default\"}\n");
    assert!(paired.err.contains("ABCD-EFGH"));
    assert!(!paired.out.contains(&token()) && !paired.err.contains(&token()) && !paired.err.contains("fake-secret"));
    assert_eq!(paired.pauses, 2);
    assert_eq!(
        fake.calls()[0].body,
        "{\"action\":\"request\",\"name\":\"test-machine\"}"
    );
    let status = execute(&fake, &["status", "--config", config]);
    assert_eq!(status.out, "{\"unlocked\":true}\n");
}

#[test]
fn denied_pairing_exits_two_and_writes_nothing() {
    let dir = TempDir::new();
    let path = dir.path().join("client.json");
    let fake = Rc::new(
        FakeTransport::new()
            .reply(200, OPENED)
            .reply(200, "{\"state\":\"denied\"}"),
    );
    let outcome = execute(
        &fake,
        &[
            "pair-request",
            "https://vault.example.test",
            "Box",
            "--config",
            path.to_str().unwrap(),
        ],
    );
    assert_eq!(outcome.exit, ExitCode::Refused);
    assert!(outcome.err.contains("pairing-denied"));
    assert!(!path.exists());
    assert_eq!(fake.calls()[0].body, "{\"action\":\"request\",\"name\":\"Box\"}");
}

#[test]
fn expired_pairing_exits_two() {
    let dir = TempDir::new();
    let path = dir.path().join("client.json");
    let pending = "{\"state\":\"pending\",\"expiresInSeconds\":1}";
    let short =
        "{\"requestId\":\"r1\",\"secret\":\"fake-secret\",\"verification\":\"ABCD-EFGH\",\"expiresInSeconds\":2}";
    let fake = Rc::new(FakeTransport::new().reply(200, short).reply(200, pending));
    let outcome = execute(
        &fake,
        &[
            "pair-request",
            "https://vault.example.test",
            "--config",
            path.to_str().unwrap(),
        ],
    );
    assert_eq!(outcome.exit, ExitCode::Refused);
    assert!(outcome.err.contains("pairing-expired"));
    assert!(!path.exists());
}

#[test]
fn pair_request_refuses_a_non_https_address() {
    let dir = TempDir::new();
    let path = dir.path().join("client.json");
    let fake = Rc::new(FakeTransport::new());
    let outcome = execute(
        &fake,
        &[
            "pair-request",
            "http://vault.example.test",
            "--config",
            path.to_str().unwrap(),
        ],
    );
    assert_eq!(outcome.exit, ExitCode::Usage);
    assert_eq!(fake.calls().len(), 0);
}
