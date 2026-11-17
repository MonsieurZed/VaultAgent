//! Route `elevation-disable` de bout en bout : contexte `disable-elevation`, preuve de la clé enrôlée, retrait.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::service::{Client, ORIGIN, Service, VaultState};
use serde_json::{Value, json};
use vaultagent_core::encoding::base64_decode;
use vaultagent_core::{RsaPrivateKey, SeededEntropy, decision_message, import_pkcs8, sign_pss};

struct Keys {
    approver: RsaPrivateKey,
    elevation: RsaPrivateKey,
    other: RsaPrivateKey,
}

/// Clés factices de la fixture partagée avec les tests du coeur.
fn fixture() -> Value {
    serde_json::from_str(include_str!("../../vaultagent-core/tests/fixtures/unlock.json")).unwrap()
}

/// Texte d'un champ de la fixture.
fn field(data: &Value, owner: &str, name: &str) -> String {
    data[owner][name].as_str().unwrap().to_owned()
}

/// Clé privée d'un propriétaire de la fixture.
fn private(data: &Value, owner: &str) -> RsaPrivateKey {
    import_pkcs8(&base64_decode(&field(data, owner, "privateKeyPkcs8")).unwrap()).unwrap()
}

/// Service dont l'unique appareil est approbateur, avec ou sans clé d'élévation enrôlée.
fn service(enrolled: bool) -> (Service, Keys) {
    let service = Service::new(VaultState::Unlocked, Client::Approver);
    let data = fixture();
    let vault = service.vaults.get(None).unwrap();
    let mut record = vault.store.devices()[0].clone();
    record.approver_public_key = Some(field(&data, "approver", "publicKey"));
    record.elevation_public_key = enrolled.then(|| field(&data, "elevation", "publicKey"));
    vault.store.update_devices(|_| Ok(vec![record])).unwrap();
    let keys = Keys {
        approver: private(&data, "approver"),
        elevation: private(&data, "elevation"),
        other: private(&data, "other"),
    };
    (service, keys)
}

/// Signature du message de décision `disable-elevation`.
fn sign(key: &RsaPrivateKey, challenge: &str) -> String {
    let message = decision_message(ORIGIN, "default", "disable-elevation", challenge, true, false);
    sign_pss(key, &message, &mut SeededEntropy::new(3)).unwrap()
}

/// Ouvre le contexte de retrait et rend son défi.
fn begin(service: &Service) -> String {
    let (status, body) = service.call("elevation-begin", &json!({"purpose": "disable"}));
    assert_eq!((status, body["requestId"].as_str()), (200, Some("disable-elevation")));
    body["challenge"].as_str().unwrap().to_owned()
}

/// Appelle `elevation-disable` avec les deux signatures données.
fn disable(service: &Service, approver: &str, elevation: &str) -> (u16, Value) {
    service.call(
        "elevation-disable",
        &json!({"signature": approver, "elevationSignature": elevation}),
    )
}

#[test]
fn disable_removes_the_key_and_the_context_is_single_use() {
    let (service, keys) = service(true);
    let challenge = begin(&service);
    let (status, body) = disable(
        &service,
        &sign(&keys.approver, &challenge),
        &sign(&keys.elevation, &challenge),
    );
    assert_eq!(status, 200);
    assert_eq!(body, json!({"elevationEnabled": false, "vaultId": "default"}));
    let vault = service.vaults.get(None).unwrap();
    assert_eq!(vault.store.devices()[0].elevation_public_key, None);
    let (_, state) = service.call("status", &json!({}));
    assert_eq!(state["approvalSecurity"]["elevationEnabled"], false);
    let replay = disable(
        &service,
        &sign(&keys.approver, &challenge),
        &sign(&keys.elevation, &challenge),
    );
    assert_eq!(
        (replay.0, replay.1["error"].as_str()),
        (400, Some("elevation-not-enrolled"))
    );
}

#[test]
fn disable_is_refused_for_each_cause_and_keeps_the_key() {
    let (service, keys) = service(true);
    let (status, none) = disable(&service, "x", "x");
    assert_eq!((status, none["error"].as_str()), (400, Some("request-expired")));
    let challenge = begin(&service);
    let approver = sign(&keys.approver, &challenge);
    let wrong_elevation = disable(&service, &approver, &sign(&keys.other, &challenge));
    assert_eq!(wrong_elevation.1["error"], "approval-refused");
    let wrong_approver = disable(
        &service,
        &sign(&keys.other, &challenge),
        &sign(&keys.elevation, &challenge),
    );
    assert_eq!(wrong_approver.1["error"], "approval-refused");
    let vault = service.vaults.get(None).unwrap();
    assert!(vault.store.devices()[0].elevation_public_key.is_some());
}

#[test]
fn begin_refuses_a_non_approver_and_an_absent_key() {
    let paired = Service::new(VaultState::Unlocked, Client::Paired);
    let (_, body) = paired.call("elevation-begin", &json!({"purpose": "disable"}));
    assert_eq!(body["error"], "approver-required");
    let (none, _) = service(false);
    let (_, body) = none.call("elevation-begin", &json!({"purpose": "disable"}));
    assert_eq!(body["error"], "elevation-not-enrolled");
    let (status, body) = none.call("elevation-begin", &json!({"purpose": "other"}));
    assert_eq!((status, body["error"].as_str()), (400, Some("request-refused")));
}

#[test]
fn begin_without_purpose_still_opens_the_enrollment() {
    let (service, _) = service(false);
    for body in [json!({}), json!({"purpose": "enroll"})] {
        let (status, answer) = service.call("elevation-begin", &body);
        assert_eq!((status, answer["requestId"].as_str()), (200, Some("enroll-elevation")));
    }
}
