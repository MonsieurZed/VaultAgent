//! Interopérabilité avec Node : vecteurs `fixtures/crypto.json` produits par `node:crypto` (clés RSA factices jetables).
//! Couvre `protect`, `seal`, l'ouverture d'enveloppe, RSA-PSS, le message de décision et les refus de clés.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use rsa::RsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use serde_json::Value;
use vaultagent_core::encoding::{base64_decode, base64_encode, hex_encode};
use vaultagent_core::{
    Envelope, ErrorCode, Protected, SeededEntropy, decision_message, open_envelope, protect, rsa_public_key, seal,
    unprotect, verify_pss,
};

const FIXTURE: &str = include_str!("fixtures/crypto.json");

/// Fixture lue une fois.
fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

/// Texte d'une valeur JSON.
fn text(value: &Value) -> &str {
    value.as_str().unwrap()
}

/// Clé machine factice `00 01 .. 1f`, vérifiée contre la fixture.
fn machine_key(data: &Value) -> Vec<u8> {
    let key: Vec<u8> = (0..32u8).collect();
    assert_eq!(hex_encode(&key), text(&data["protect"]["keyHex"]));
    key
}

/// Clé privée RSA factice de la fixture.
fn private_key(data: &Value) -> RsaPrivateKey {
    let der = base64_decode(text(&data["seal"]["privateKeyPkcs8"])).unwrap();
    RsaPrivateKey::from_pkcs8_der(&der).unwrap()
}

#[test]
fn unprotect_reads_a_value_protected_by_node() {
    let data = fixture();
    let value: Protected = serde_json::from_value(data["protect"]["value"].clone()).unwrap();
    let clear = unprotect(&machine_key(&data), &value).unwrap();
    assert_eq!(
        String::from_utf8(clear.to_vec()).unwrap(),
        text(&data["protect"]["clear"])
    );
}

#[test]
fn protect_round_trips_and_uses_the_node_format() {
    let data = fixture();
    let key = machine_key(&data);
    let mut entropy = SeededEntropy::new(5);
    let first = protect(&key, "été".as_bytes(), &mut entropy).unwrap();
    let second = protect(&key, "été".as_bytes(), &mut entropy).unwrap();
    assert_ne!(first.nonce, second.nonce);
    assert_eq!(base64_decode(&first.nonce).unwrap().len(), 12);
    assert_eq!(base64_decode(&first.tag).unwrap().len(), 16);
    assert_eq!(unprotect(&key, &first).unwrap().as_slice(), "été".as_bytes());
    let json = serde_json::to_value(&first).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 3);
    assert_eq!(
        unprotect(&key, &protect(&key, b"", &mut entropy).unwrap())
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn unprotect_refuses_wrong_key_tamper_and_malformed_fields() {
    let data = fixture();
    let key = machine_key(&data);
    let good: Protected = serde_json::from_value(data["protect"]["value"].clone()).unwrap();
    let mut wrong = key.clone();
    wrong[0] ^= 1;
    assert_eq!(unprotect(&wrong, &good), Err(ErrorCode::InvalidKey));
    assert_eq!(unprotect(&key[..16], &good), Err(ErrorCode::InvalidKey));
    let mut raw = base64_decode(&good.data).unwrap();
    raw[0] ^= 1;
    let tampered = Protected {
        data: base64_encode(&raw),
        ..good.clone()
    };
    assert_eq!(unprotect(&key, &tampered), Err(ErrorCode::InvalidKey));
    for broken in [
        Protected {
            nonce: "AAAA".into(),
            ..good.clone()
        },
        Protected {
            tag: "AAAA".into(),
            ..good.clone()
        },
        Protected {
            data: "***".into(),
            ..good.clone()
        },
    ] {
        assert_eq!(unprotect(&key, &broken), Err(ErrorCode::InvalidKey));
    }
}

#[test]
fn open_envelope_reads_a_seal_made_by_node() {
    let data = fixture();
    let envelope: Envelope = serde_json::from_value(data["seal"]["envelope"].clone()).unwrap();
    let clear = open_envelope(&envelope, &private_key(&data), &mut SeededEntropy::new(1)).unwrap();
    assert_eq!(String::from_utf8(clear.to_vec()).unwrap(), text(&data["seal"]["clear"]));
}

#[test]
fn seal_round_trips_and_exposes_the_protocol_fields() {
    let data = fixture();
    let public = text(&data["seal"]["publicKey"]);
    let mut entropy = SeededEntropy::new(11);
    let first = seal("Secret factice é".as_bytes(), public, &mut entropy).unwrap();
    let second = seal("Secret factice é".as_bytes(), public, &mut entropy).unwrap();
    assert_ne!(first.encrypted_key, second.encrypted_key);
    let json = serde_json::to_value(&first).unwrap();
    let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["data", "encryptedKey", "nonce", "tag"]);
    assert_eq!(base64_decode(&first.encrypted_key).unwrap().len(), 256);
    let clear = open_envelope(&first, &private_key(&data), &mut entropy).unwrap();
    assert_eq!(clear.as_slice(), "Secret factice é".as_bytes());
}

#[test]
fn seal_is_reproducible_with_a_seeded_source() {
    let data = fixture();
    let public = text(&data["seal"]["publicKey"]);
    let first = seal(b"x", public, &mut SeededEntropy::new(8)).unwrap();
    assert_eq!(first, seal(b"x", public, &mut SeededEntropy::new(8)).unwrap());
}

#[test]
fn open_envelope_refuses_tampering_and_wrong_keys() {
    let data = fixture();
    let key = private_key(&data);
    let good: Envelope = serde_json::from_value(data["seal"]["envelope"].clone()).unwrap();
    let mut entropy = SeededEntropy::new(2);
    let mut raw = base64_decode(&good.data).unwrap();
    raw[0] ^= 1;
    let tampered = Envelope {
        data: base64_encode(&raw),
        ..good.clone()
    };
    let mut wrapped = base64_decode(&good.encrypted_key).unwrap();
    wrapped[5] ^= 1;
    for broken in [
        tampered,
        Envelope {
            encrypted_key: base64_encode(&wrapped),
            ..good.clone()
        },
        Envelope {
            encrypted_key: "***".into(),
            ..good.clone()
        },
        Envelope {
            nonce: "AAAA".into(),
            ..good.clone()
        },
        Envelope {
            tag: "AAAA".into(),
            ..good.clone()
        },
    ] {
        assert_eq!(open_envelope(&broken, &key, &mut entropy), Err(ErrorCode::InvalidKey));
    }
}

#[test]
fn seal_accepts_2048_to_4096_bits_and_refuses_the_rest() {
    let data = fixture();
    let sizes = &data["sizes"];
    for accepted in ["rsa3072", "rsa4096"] {
        assert!(
            seal(b"v", text(&sizes[accepted]), &mut SeededEntropy::new(1)).is_ok(),
            "{accepted}"
        );
    }
    for refused in ["rsa1024", "ec", "ed25519"] {
        assert_eq!(
            seal(b"v", text(&sizes[refused]), &mut SeededEntropy::new(1)),
            Err(ErrorCode::InvalidKey)
        );
    }
    for garbage in ["", "pas-une-cle", "AAAA"] {
        assert_eq!(
            seal(b"v", garbage, &mut SeededEntropy::new(1)),
            Err(ErrorCode::InvalidKey)
        );
    }
}

#[test]
fn approver_key_requires_rsa_2048_within_1024_characters() {
    let data = fixture();
    assert!(rsa_public_key(text(&data["pss"]["publicKey"])).is_ok());
    for refused in ["rsa1024", "rsa3072", "rsa4096", "ec", "ed25519"] {
        assert_eq!(
            rsa_public_key(text(&data["sizes"][refused])).err(),
            Some(ErrorCode::InvalidKey),
            "{refused}"
        );
    }
    assert_eq!(rsa_public_key("pas-une-cle").err(), Some(ErrorCode::InvalidKey));
    assert_eq!(rsa_public_key(&"A".repeat(1025)).err(), Some(ErrorCode::InvalidKey));
}

#[test]
fn pss_signature_made_by_node_verifies() {
    let data = fixture();
    let pss = &data["pss"];
    let message = decision_message(
        text(&pss["origin"]),
        text(&pss["vaultId"]),
        text(&pss["requestId"]),
        text(&pss["challenge"]),
        pss["approve"].as_bool().unwrap(),
        pss["remember"].as_bool().unwrap(),
    );
    assert_eq!(message, text(&pss["message"]));
    let ok = verify_pss(
        text(&pss["publicKey"]),
        &message,
        text(&pss["signature"]),
        ErrorCode::ApprovalRefused,
    );
    assert_eq!(ok, Ok(()));
}

#[test]
fn pss_refusals_use_the_given_failure_code() {
    let data = fixture();
    let pss = &data["pss"];
    let (key, message, signature) = (text(&pss["publicKey"]), text(&pss["message"]), text(&pss["signature"]));
    let failure = ErrorCode::ElevationRequired;
    assert_eq!(
        verify_pss(key, "autre message", signature, failure.clone()),
        Err(failure.clone())
    );
    assert_eq!(
        verify_pss(text(&pss["otherPublicKey"]), message, signature, failure.clone()),
        Err(failure.clone())
    );
    assert_eq!(
        verify_pss(key, message, text(&pss["saltEmptySignature"]), failure.clone()),
        Err(failure.clone())
    );
    assert_eq!(verify_pss(key, message, "***", failure.clone()), Err(failure.clone()));
    assert_eq!(verify_pss(key, message, "", failure.clone()), Err(failure.clone()));
    assert_eq!(verify_pss(key, message, "AAAA", failure.clone()), Err(failure));
    assert_eq!(
        verify_pss("pas-une-cle", message, signature, ErrorCode::ApprovalRefused),
        Err(ErrorCode::InvalidKey)
    );
}

#[test]
fn decision_message_matches_json_stringify() {
    let data = fixture();
    let m = &data["message"];
    let built = decision_message(
        text(&m["origin"]),
        text(&m["vaultId"]),
        text(&m["requestId"]),
        text(&m["challenge"]),
        m["approve"].as_bool().unwrap(),
        m["remember"].as_bool().unwrap(),
    );
    assert_eq!(built, text(&m["expected"]));
}
