//! Essais réels de Windows Hello. `hello-status` ne montre aucune invite et tourne toujours. Les essais ignorés
//! affichent l'invite Windows Hello sur l'écran : à lancer avec Zed, `cargo test -p vaultagent-client --test
//! hello_live -- --ignored --test-threads=1`. Ils utilisent le site de test `vaultagent-test.local`, jamais celui
//! du serveur, et ne parlent à aucun serveur.

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use serde_json::Value;
use vaultagent_client::hello_cose::credential_from_registration;
use vaultagent_client::hello_enroll::hello_status;
use vaultagent_client::hello_system::HelloSystem;
use vaultagent_client::windows_hello::WindowsHello;
use vaultagent_core::encoding::{base64_decode, hex_encode};
use vaultagent_core::{digest, hello_challenge};

const TEST_RP: &str = "vaultagent-test.local";
const TEST_ORIGIN: &str = "https://vaultagent-test.local";

#[test]
fn hello_status_reports_the_api_without_any_prompt() {
    let line = hello_status(&WindowsHello).unwrap();
    let value: Value = serde_json::from_str(&line).unwrap();
    println!("hello-status : {line}");
    assert!(value["available"].is_boolean());
    assert!(value["apiVersion"].as_u64().unwrap() > 0);
}

/// Octets d'un hexadécimal minuscule.
fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
#[ignore = "invite Windows Hello : à lancer avec Zed"]
fn register_then_sign_on_the_test_site_round_trips() {
    let registration = WindowsHello.register(TEST_RP, TEST_ORIGIN).unwrap();
    let credential = credential_from_registration(&registration, TEST_RP).unwrap();
    assert_eq!(credential.key_type, "EC2");
    let id = base64_decode(&credential.id).unwrap();
    let challenge = hello_challenge("message de test");
    let assertion = WindowsHello.sign(&id, &challenge, TEST_RP, TEST_ORIGIN).unwrap();
    assert_eq!(assertion.credential_id, id);
    let client: Value = serde_json::from_slice(&assertion.client_data).unwrap();
    assert_eq!(client["challenge"], challenge.as_str());
    assert_eq!(client["origin"], TEST_ORIGIN);
    assert_eq!(
        hex_encode(&assertion.authenticator_data[..32]),
        digest(TEST_RP.as_bytes())
    );
    assert_eq!(assertion.authenticator_data[32] & 5, 5);
    let (x, y) = (
        base64_decode(&credential.first).unwrap(),
        base64_decode(&credential.second).unwrap(),
    );
    let point = [&[4u8][..], &x, &y].concat();
    let key = VerifyingKey::from_sec1_bytes(&point).unwrap();
    let signed = [
        assertion.authenticator_data.clone(),
        unhex(&digest(&assertion.client_data)),
    ]
    .concat();
    key.verify(&signed, &Signature::from_der(&assertion.signature).unwrap())
        .unwrap();
}

#[test]
#[ignore = "invite Windows Hello : à lancer avec Zed, annuler l'invite"]
fn cancelled_prompt_is_reported_as_cancelled() {
    let error = WindowsHello.register(TEST_RP, TEST_ORIGIN).unwrap_err();
    assert_eq!(error.code(), "hello-cancelled");
}
