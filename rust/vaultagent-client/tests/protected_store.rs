//! Stockage protégé par DPAPI de bout en bout (Windows) : association, relecture sans `--config`, clé d'approbateur.
//! Le dossier de données est toujours un dossier temporaire injecté, jamais celui de l'utilisateur.

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::rc::Rc;

use common::{TempDir, execute_in, token};
use vaultagent_client::exit_code::ExitCode;
use vaultagent_client::fake_transport::FakeTransport;

const OPENED: &str =
    "{\"requestId\":\"r1\",\"secret\":\"fake-secret\",\"verification\":\"ABCD-EFGH\",\"expiresInSeconds\":30}";

/// Faux serveur qui approuve l'association puis répond au statut.
fn approving_server() -> Rc<FakeTransport> {
    let approved = format!(
        "{{\"state\":\"approved\",\"token\":\"{}\",\"deviceId\":\"d\",\"vaultId\":\"default\"}}",
        token()
    );
    Rc::new(
        FakeTransport::new()
            .reply(200, OPENED)
            .reply(200, &approved)
            .reply(200, "{\"unlocked\":true}"),
    )
}

#[test]
fn pairing_without_config_writes_the_protected_file_and_status_reads_it() {
    let dir = TempDir::new();
    let data = dir.path().join("VaultAgent");
    let fake = approving_server();
    let paired = execute_in(&fake, &["pair-request", "https://vault.example.test"], b"", Some(&data));
    assert_eq!(paired.exit, ExitCode::Success);
    let text = std::fs::read_to_string(data.join("remote-client.json")).unwrap();
    assert!(text.contains("\"protectedToken\"") && !text.contains(&token()));
    assert!(!data.join("approval-key.dpapi").exists());
    assert_eq!(
        fake.calls()[0].body,
        "{\"action\":\"request\",\"name\":\"test-machine\"}"
    );
    let status = execute_in(&fake, &["status"], b"", Some(&data));
    assert_eq!(status.out, "{\"unlocked\":true}\n");
    assert_eq!(
        fake.calls()[2].bearer.as_deref(),
        Some(format!("Bearer {}", token()).as_str())
    );
}

#[test]
fn approver_option_sends_the_public_key_and_creates_the_key_file() {
    let dir = TempDir::new();
    let data = dir.path().join("VaultAgent");
    let fake = approving_server();
    let args = ["pair-request", "https://vault.example.test", "--approver"];
    let paired = execute_in(&fake, &args, b"", Some(&data));
    assert_eq!(paired.exit, ExitCode::Success);
    assert!(data.join("approval-key.dpapi").exists());
    let body = &fake.calls()[0].body;
    assert!(body.starts_with("{\"action\":\"request\",\"name\":\"test-machine\",\"approverPublicKey\":\"MII"));
    assert!(!paired.out.contains("approverPublicKey") && !paired.err.contains("MII"));
}

#[test]
fn approver_option_with_config_is_a_usage_error() {
    let dir = TempDir::new();
    let path = dir.path().join("client.json");
    let fake = Rc::new(FakeTransport::new());
    let args = [
        "pair-request",
        "https://vault.example.test",
        "--approver",
        "--config",
        path.to_str().unwrap(),
    ];
    let outcome = execute_in(&fake, &args, b"", Some(&dir.path().join("VaultAgent")));
    assert_eq!(outcome.exit, ExitCode::Usage);
    assert_eq!(fake.calls().len(), 0);
}

#[test]
fn missing_protected_file_is_a_config_error() {
    let dir = TempDir::new();
    let fake = Rc::new(FakeTransport::new());
    let outcome = execute_in(&fake, &["status"], b"", Some(&dir.path().join("VaultAgent")));
    assert_eq!(outcome.exit, ExitCode::Usage);
    assert!(outcome.err.contains("\"error\":\"config\""));
}
