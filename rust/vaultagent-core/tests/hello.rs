//! Vérification Hello : preuves EC2 et RSA produites par Node (clés jetables), variantes invalides, justificatif et
//! encodages refusés.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value,
    clippy::type_complexity
)]

mod common;

use common::{challenge_of, fixture};
use serde_json::Value;
use vaultagent_core::{
    ErrorCode, HelloCredential, HelloKey, HelloProof, hello_challenge, hello_public_key, verify_hello,
};

/// Justificatif et preuve d'un type de clé de la fixture.
fn sample(kind: &str) -> (HelloCredential, HelloProof, Value) {
    let data = fixture();
    let section = &data[kind];
    let credential = serde_json::from_value(section["credential"].clone()).unwrap();
    let proof = serde_json::from_value(section["proof"].clone()).unwrap();
    (credential, proof, data["helloMessage"].clone())
}

/// Variante nommée de la fixture.
fn variant(kind: &str, name: &str) -> HelloProof {
    serde_json::from_value(fixture()[kind]["variants"][name].clone()).unwrap()
}

#[test]
fn challenge_is_base64url_of_the_sha256() {
    assert_eq!(
        hello_challenge("transaction-factice"),
        challenge_of("transaction-factice")
    );
    assert!(!hello_challenge("x").contains('='));
}

#[test]
fn node_proofs_verify_for_ec2_and_rsa() {
    for kind in ["ec2", "rsa"] {
        let (credential, proof, message) = sample(kind);
        let message = message.as_str().unwrap();
        assert_eq!(verify_hello(Some(&credential), Some(&proof), message), Ok(()), "{kind}");
        assert_eq!(
            verify_hello(Some(&credential), Some(&proof), &format!("{message}autre")),
            Err(ErrorCode::HelloInvalid),
            "{kind}"
        );
    }
}

#[test]
fn every_invalid_variant_is_refused() {
    let names = [
        "flagsPresenceOnly",
        "flagsVerifiedOnly",
        "wrongRp",
        "wrongOrigin",
        "wrongChallenge",
        "createType",
        "crossOrigin",
        "badSignature",
    ];
    for kind in ["ec2", "rsa"] {
        let (credential, _, message) = sample(kind);
        for name in names {
            assert_eq!(
                verify_hello(Some(&credential), Some(&variant(kind, name)), message.as_str().unwrap()),
                Err(ErrorCode::HelloInvalid),
                "{kind} {name}"
            );
        }
    }
}

#[test]
fn missing_proof_is_required_but_a_missing_credential_is_invalid() {
    let (credential, proof, message) = sample("ec2");
    let message = message.as_str().unwrap();
    assert_eq!(
        verify_hello(Some(&credential), None, message),
        Err(ErrorCode::HelloRequired)
    );
    assert_eq!(verify_hello(None, Some(&proof), message), Err(ErrorCode::HelloInvalid));
    assert_eq!(verify_hello(None, None, message), Err(ErrorCode::HelloInvalid));
}

#[test]
fn proof_fields_are_decoded_strictly() {
    let (credential, proof, message) = sample("ec2");
    let message = message.as_str().unwrap();
    let mutations: Vec<Box<dyn Fn(&mut HelloProof)>> = vec![
        Box::new(|p| p.credential_id = String::new()),
        Box::new(|p| p.client_data = "***".to_owned()),
        Box::new(|p| p.client_data.push('A')),
        Box::new(|p| p.authenticator_data = "AAAA".to_owned()),
        Box::new(|p| p.signature = "A".repeat(2100)),
        Box::new(|p| p.signature = String::new()),
        Box::new(|p| p.client_data = "e30=".to_owned()),
        Box::new(|p| p.credential_id = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned()),
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut broken = proof.clone();
        mutate(&mut broken);
        assert_eq!(
            verify_hello(Some(&credential), Some(&broken), message),
            Err(ErrorCode::HelloInvalid),
            "mutation {index}"
        );
    }
    assert_eq!(
        verify_hello(Some(&credential), Some(&HelloProof::default()), message),
        Err(ErrorCode::HelloInvalid)
    );
}

#[test]
fn credential_shapes_follow_the_key_type_rules() {
    let (credential, ..) = sample("ec2");
    assert!(matches!(hello_public_key(Some(&credential)), Ok(HelloKey::Ec(_))));
    let (rsa, ..) = sample("rsa");
    assert!(matches!(hello_public_key(Some(&rsa)), Ok(HelloKey::Rsa(_))));
    let tweaks: Vec<Box<dyn Fn(&mut HelloCredential)>> = vec![
        Box::new(|c| c.id = "AAAA".to_owned()),
        Box::new(|c| c.key_type = "OKP".to_owned()),
        Box::new(|c| c.key_type = "RSA".to_owned()),
        Box::new(|c| c.first = c.second.clone()),
        Box::new(|c| c.second = "AAAA".to_owned()),
        Box::new(|c| c.first = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned()),
        Box::new(|c| c.id = String::new()),
    ];
    for (index, tweak) in tweaks.iter().enumerate() {
        let mut broken = credential.clone();
        tweak(&mut broken);
        assert_eq!(
            hello_public_key(Some(&broken)).err(),
            Some(ErrorCode::HelloInvalid),
            "tweak {index}"
        );
    }
    let mut short_exponent = rsa.clone();
    short_exponent.second = "AQABAAE=".to_owned();
    assert_eq!(
        hello_public_key(Some(&short_exponent)).err(),
        Some(ErrorCode::HelloInvalid)
    );
    assert_eq!(hello_public_key(None).err(), Some(ErrorCode::HelloInvalid));
}

#[test]
fn proof_of_another_credential_id_is_refused() {
    let (credential, proof, message) = sample("ec2");
    let (other, ..) = sample("rsa");
    let mut swapped = proof;
    swapped.credential_id = other.id;
    assert_eq!(
        verify_hello(Some(&credential), Some(&swapped), message.as_str().unwrap()),
        Err(ErrorCode::HelloInvalid)
    );
}
