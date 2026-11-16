//! Route `seal` : enveloppe, contrôles du corps, attente du TOTP et refus pendant une opération.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::service::{Client, ITEM_ID, REVISION, Service, VaultState, demo_item, item_reply};
use serde_json::{Value, json};
use vaultagent_core::Uuid;
use vaultagent_engine::{Reply, Secret};
use vaultagent_server::{Context, Pause, route};

/// Aide de test : public key.
fn public_key() -> String {
    include_str!("fixtures/rsa-public.txt").trim().to_owned()
}

/// Aide de test : body.
fn body(field: &str) -> Value {
    json!({"itemId": ITEM_ID, "field": field, "origin": "https://example.test", "publicKey": public_key()})
}

/// Aide de test : service.
fn service() -> Service {
    Service::new(VaultState::Unlocked, Client::Paired)
}

#[test]
fn envelope_has_the_documented_sizes_and_carries_no_clear_value() {
    let service = service();
    service.script(vec![Reply::Unit, item_reply(demo_item(REVISION, "Compte"))]);
    let (status, envelope) = service.call("seal", &body("password"));
    assert_eq!(status, 200);
    let length = |key: &str| envelope[key].as_str().unwrap().len();
    assert_eq!(
        (length("encryptedKey"), length("nonce"), length("tag"), length("data")),
        (344, 16, 24, 16)
    );
    assert!(!envelope.to_string().contains("Fictif-42!"));
    assert_eq!(service.engine.call_names()[1..], ["sync", "item"]);
}

#[test]
fn body_is_validated_before_the_engine_is_called() {
    let service = service();
    let mut cases = vec![];
    for (key, value) in [
        ("field", json!("item")),
        ("origin", json!("http://example.test")),
        ("origin", json!("https://example.test/chemin")),
        ("origin", json!("https://user@example.test")),
        ("itemId", json!("x")),
    ] {
        let mut changed = body("password");
        changed[key] = value;
        cases.push(changed);
    }
    for case in cases {
        assert_eq!(
            service.call("seal", &case),
            (400, json!({"error": "invalid-field"})),
            "{case}"
        );
    }
    assert_eq!(service.engine.call_names(), vec!["connect"]);
}

#[test]
fn origin_must_belong_to_the_item_and_the_value_must_exist() {
    let service = service();
    let mut other = body("password");
    other["origin"] = json!("https://autre.example.test");
    let mut bare = demo_item(REVISION, "Sans identifiant");
    bare.login.as_mut().unwrap().username = None;
    service.script(vec![
        Reply::Unit,
        item_reply(demo_item(REVISION, "Compte")),
        Reply::Unit,
        item_reply(bare),
    ]);
    assert_eq!(service.call("seal", &other), (403, json!({"error": "origin-refused"})));
    assert_eq!(
        service.call("seal", &body("username")),
        (400, json!({"error": "field-missing"}))
    );
}

#[test]
fn a_bad_public_key_is_refused() {
    let service = service();
    service.script(vec![Reply::Unit, item_reply(demo_item(REVISION, "Compte"))]);
    let mut weak = body("password");
    weak["publicKey"] = json!("AAAA");
    assert_eq!(service.call("seal", &weak), (400, json!({"error": "invalid-key"})));
}

#[test]
fn totp_waits_for_the_next_period_when_less_than_five_seconds_remain() {
    let service = service();
    service.clock.advance(27_000);
    service.script(vec![
        Reply::Unit,
        item_reply(demo_item(REVISION, "Compte")),
        Reply::Secret(Secret::from("123456")),
    ]);
    let (status, _) = service.call("seal", &body("totp"));
    assert_eq!((status, service.pause.waits()), (200, vec![3_100]));
    assert_eq!(service.engine.call_names().last(), Some(&"totp"));

    let early = service_with_clock(10_000);
    early.script(vec![
        Reply::Unit,
        item_reply(demo_item(REVISION, "Compte")),
        Reply::Secret(Secret::from("123456")),
    ]);
    assert_eq!(early.call("seal", &body("totp")).0, 200);
    assert_eq!(early.pause.waits(), Vec::<u64>::new());
}

/// Aide de test : service with clock.
fn service_with_clock(advance: u64) -> Service {
    let service = service();
    service.clock.advance(advance);
    service
}

#[test]
fn totp_period_outside_15_to_120_seconds_is_refused() {
    let service = service();
    let mut item = demo_item(REVISION, "Compte");
    item.login.as_mut().unwrap().totp = Some(Secret::from("otpauth://totp/x?secret=ABC&period=200"));
    service.script(vec![Reply::Unit, item_reply(item)]);
    assert_eq!(
        service.call("seal", &body("totp")),
        (400, json!({"error": "totp-period"}))
    );
}

struct Hook(Box<dyn Fn() + Send + Sync>);

impl Pause for Hook {
    /// Aide de test : sleep ms.
    fn sleep_ms(&self, _milliseconds: u64) {
        (self.0)();
    }
}

/// Aide de test : with hook.
fn with_hook(service: &Service, action: impl Fn() + Send + Sync + 'static) -> Context {
    Context {
        pause: Arc::new(Hook(Box::new(action))),
        ..service.context.clone()
    }
}

/// Aide de test : totp script.
fn totp_script(service: &Service) {
    service.clock.advance(27_000);
    service.script(vec![
        Reply::Unit,
        item_reply(demo_item(REVISION, "Compte")),
        Reply::Secret(Secret::from("123456")),
    ]);
}

#[test]
fn locking_during_the_wait_returns_no_envelope() {
    let service = service();
    totp_script(&service);
    let vault = service.vaults.get(None).unwrap();
    let context = with_hook(&service, move || vault.session.lock());
    let response = route(&service.request("seal", &body("totp")), &context);
    assert_eq!(response.status, 423);
    assert!(!String::from_utf8_lossy(&response.body).contains("encryptedKey"));
    assert!(!service.engine.call_names().contains(&"totp"));
}

#[test]
fn revoking_the_client_during_the_wait_returns_no_envelope() {
    let service = service();
    totp_script(&service);
    let vault = service.vaults.get(None).unwrap();
    let context = with_hook(&service, move || {
        vault.store.update_devices(|_| Ok(Vec::new())).unwrap();
    });
    let response = route(&service.request("seal", &body("totp")), &context);
    assert_eq!(response.status, 403);
    assert!(!String::from_utf8_lossy(&response.body).contains("encryptedKey"));
    assert!(Uuid::parse(ITEM_ID).is_some());
}
