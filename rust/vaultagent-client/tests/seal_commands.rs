//! `seal` et `fill_with` de bout en bout contre un faux serveur qui scelle une valeur sentinelle.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Duration;

use common::{TempDir, execute};
use serde_json::Value;
use vaultagent_client::config::ClientConfig;
use vaultagent_client::error::ClientError;
use vaultagent_client::fake_transport::FakeTransport;
use vaultagent_client::fill::{fill_with, fill_with_key};
use vaultagent_client::server_address::ServerAddress;
use vaultagent_client::token::Token;
use vaultagent_client::transport::{Response, Transport, TransportError};
use vaultagent_core::{RsaPrivateKey, SystemEntropy, generate_rsa, public_key_spki, seal};

const SENTINEL: &str = "SENTINEL-not-a-real-secret-4242";
const ITEM: &str = "0000000a-0000-4000-8000-00000000000c";
const ORIGIN: &str = "https://example.test";
const ENVELOPE: &str = "{\"encryptedKey\":\"AAAA\",\"nonce\":\"BBBB\",\"tag\":\"CCCC\",\"data\":\"DDDD\"}";

/// Clé privée de test, générée une seule fois pour tout le fichier (la génération est lente hors optimisation).
fn shared_key() -> &'static RsaPrivateKey {
    static KEY: OnceLock<RsaPrivateKey> = OnceLock::new();
    KEY.get_or_init(|| generate_rsa(2048, &mut SystemEntropy).unwrap())
}

/// Clé publique SPKI valide, dérivée de la clé partagée.
fn public_key() -> &'static str {
    static KEY: OnceLock<String> = OnceLock::new();
    KEY.get_or_init(|| public_key_spki(shared_key()).unwrap())
}

/// Faux serveur qui scelle la sentinelle pour la clé reçue et garde le corps de la requête.
struct Sealing {
    bodies: Rc<RefCell<Vec<String>>>,
}

impl Transport for Sealing {
    /// Lit `publicKey`, scelle la sentinelle et rend l'enveloppe.
    fn post(&self, _: &str, _: Option<&str>, body: &[u8], _: Duration) -> Result<Response, TransportError> {
        self.bodies
            .borrow_mut()
            .push(String::from_utf8_lossy(body).into_owned());
        let value: Value = serde_json::from_slice(body).unwrap();
        let key = value["publicKey"].as_str().unwrap();
        let envelope = seal(SENTINEL.as_bytes(), key, &mut SystemEntropy).unwrap();
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

#[test]
fn fill_with_hands_the_value_to_the_sink_only() {
    let bodies = Rc::new(RefCell::new(Vec::new()));
    let transport = Sealing {
        bodies: Rc::clone(&bodies),
    };
    let seen = RefCell::new(String::new());
    let report = fill_with(&transport, &config(), None, ITEM, "password", ORIGIN, |value| {
        seen.borrow_mut().push_str(value.as_str());
        Ok(())
    })
    .unwrap();
    assert_eq!(seen.into_inner(), SENTINEL);
    let printed = serde_json::to_string(&report).unwrap();
    assert_eq!(
        printed,
        "{\"filled\":true,\"field\":\"password\",\"clipboardUsed\":false}"
    );
    assert!(!printed.contains(SENTINEL));
    let sent = bodies.borrow();
    assert_eq!(sent.len(), 1);
    assert!(!sent[0].contains(SENTINEL));
    let body: Value = serde_json::from_str(&sent[0]).unwrap();
    assert_eq!(body["itemId"], ITEM);
    assert_eq!(body["origin"], ORIGIN);
    assert_eq!(body["field"], "password");
}

#[test]
fn fill_with_propagates_the_sink_error() {
    let transport = Sealing {
        bodies: Rc::new(RefCell::new(Vec::new())),
    };
    let result = fill_with_key(
        &transport,
        &config(),
        None,
        (ITEM, "totp", ORIGIN),
        shared_key(),
        |_| Err(ClientError::Usage("Sink refused.".to_owned())),
    );
    assert_eq!(result.unwrap_err(), ClientError::Usage("Sink refused.".to_owned()));
}

#[test]
fn fill_with_refuses_bad_targets_before_any_request() {
    let fake = FakeTransport::new();
    let cases = [
        (ITEM, "notes", ORIGIN),
        ("not-a-uuid", "password", ORIGIN),
        (ITEM, "password", "http://example.test"),
        (ITEM, "password", "https://example.test/login"),
        (ITEM, "password", "https://example.test/"),
    ];
    for (item, field, origin) in cases {
        let result = fill_with(&fake, &config(), None, item, field, origin, |_| Ok(()));
        assert!(matches!(result, Err(ClientError::Usage(_))), "{item} {field} {origin}");
    }
    assert_eq!(fake.calls().len(), 0);
}

#[test]
fn fill_with_reports_a_server_refusal_and_an_unreadable_envelope() {
    let refused = FakeTransport::new().reply(403, "{\"error\":\"origin-refused\"}");
    let error = fill_with_key(
        &refused,
        &config(),
        None,
        (ITEM, "password", ORIGIN),
        shared_key(),
        |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code(), "origin-refused");
    let garbled = FakeTransport::new().reply(200, ENVELOPE);
    let error = fill_with_key(
        &garbled,
        &config(),
        None,
        (ITEM, "password", ORIGIN),
        shared_key(),
        |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code(), "bad-response");
}

/// Exécute `seal` avec une configuration valide.
fn run(fake: FakeTransport, args: &[&str]) -> (common::Outcome, Rc<FakeTransport>) {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(fake);
    let mut all: Vec<&str> = args.to_vec();
    all.extend(["--config", config.to_str().unwrap()]);
    let outcome = execute(&fake, &all);
    (outcome, fake)
}

#[test]
fn seal_command_returns_the_envelope_untouched() {
    let key = public_key();
    let (outcome, fake) = run(
        FakeTransport::new().reply(200, ENVELOPE),
        &["seal", ITEM, "password", ORIGIN, "--public-key", key],
    );
    assert_eq!(outcome.exit.code(), 0);
    assert_eq!(outcome.out.trim(), ENVELOPE);
    assert_eq!(outcome.err, "");
    let calls = fake.calls();
    assert_eq!(calls[0].path, "/api/agent/seal");
    let body: Value = serde_json::from_str(&calls[0].body).unwrap();
    assert_eq!(body["publicKey"], key);
    assert_eq!(body["itemId"], ITEM);
}

#[test]
fn seal_command_passes_the_vault_choice() {
    let (_, fake) = run(
        FakeTransport::new().reply(200, ENVELOPE),
        &[
            "--vault",
            "second",
            "seal",
            ITEM,
            "username",
            ORIGIN,
            "--public-key",
            public_key(),
        ],
    );
    let body: Value = serde_json::from_str(&fake.calls()[0].body).unwrap();
    assert_eq!(body["vaultId"], "second");
}

#[test]
fn seal_command_refuses_invalid_arguments_with_exit_one() {
    let key = public_key();
    let cases: Vec<Vec<&str>> = vec![
        vec!["seal", ITEM, "notes", ORIGIN, "--public-key", key],
        vec!["seal", "nope", "password", ORIGIN, "--public-key", key],
        vec!["seal", ITEM, "password", "http://example.test", "--public-key", key],
        vec!["seal", ITEM, "password", "https://example.test/x", "--public-key", key],
        vec!["seal", ITEM, "password", ORIGIN, "--public-key", "bm90IGEga2V5"],
        vec!["seal", ITEM, "password", ORIGIN, "--public-key", "%%%"],
    ];
    for args in cases {
        let (outcome, fake) = run(FakeTransport::new(), &args);
        assert_eq!(outcome.exit.code(), 1, "{args:?}");
        assert_eq!(outcome.out, "");
        assert!(outcome.err.contains("\"version\":1"));
        assert_eq!(fake.calls().len(), 0);
    }
}

#[test]
fn seal_command_relays_a_locked_vault_as_exit_four() {
    let (outcome, _) = run(
        FakeTransport::new().reply(423, "{\"error\":\"locked\"}"),
        &["seal", ITEM, "password", ORIGIN, "--public-key", public_key()],
    );
    assert_eq!(outcome.exit.code(), 4);
    assert_eq!(outcome.out, "");
}
