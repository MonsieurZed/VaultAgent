//! Demandes d'association : ouverture, liste, approbation, refus, sondage, limites, expiration et formes JSON.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::device_list::authenticate;
use vaultagent_core::devices::enroll_named;
use vaultagent_core::{ErrorCode, Opened, PairRequests, Polled, SeededEntropy};

const PROFILE: &str = "profil-factice";

/// Clé publique d'approbateur factice de la fixture.
fn approver_key() -> String {
    let data: Value = serde_json::from_str(include_str!("fixtures/crypto.json")).unwrap();
    data["pss"]["publicKey"].as_str().unwrap().to_owned()
}

/// Ouvre une demande sans clé d'approbateur.
fn open(requests: &mut PairRequests, name: &str, now: u64, entropy: &mut SeededEntropy) -> Opened {
    requests.open(name, None, now, entropy).unwrap()
}

#[test]
fn approved_request_hands_the_token_once() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(1));
    let opened = open(&mut requests, "  Agent\u{7} Linux  ", 0, &mut entropy);
    let code = opened.verification.as_bytes();
    assert_eq!(code.len(), 9);
    assert_eq!(code[4], b'-');
    let alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    assert!(
        opened
            .verification
            .chars()
            .filter(|c| *c != '-')
            .all(|c| alphabet.contains(c))
    );
    assert_eq!(
        (opened.request_id.len(), opened.secret.len(), opened.expires_in_seconds),
        (22, 43, 300)
    );
    let pending = requests.poll(&opened.request_id, &opened.secret, 1000).unwrap();
    assert_eq!(
        pending,
        Polled::Pending {
            expires_in_seconds: 299
        }
    );
    let listed = requests.list(1000);
    assert_eq!((listed[0].name.as_str(), listed[0].approver), ("Agent Linux", false));
    assert_eq!(listed[0].verification, opened.verification);
    assert!(!serde_json::to_string(&listed).unwrap().contains(&opened.secret));
    let target = requests.target(&opened.request_id, 1000).unwrap();
    assert_eq!(
        (target.name.as_str(), target.approver_public_key),
        ("Agent Linux", None)
    );
    let (records, enrolled) = enroll_named(&target.name, Some(PROFILE), None, 1000, &[], &mut entropy).unwrap();
    requests
        .approve(&opened.request_id, 1000, "default", &enrolled)
        .unwrap();
    assert_eq!(requests.list(1000), Vec::new());
    let polled = requests.poll(&opened.request_id, &opened.secret, 2000).unwrap();
    let Polled::Approved {
        token,
        device_id,
        vault_id,
    } = polled
    else {
        panic!("approuvée attendue");
    };
    assert_eq!(
        (vault_id.as_str(), device_id.as_str()),
        ("default", enrolled.device_id.as_str())
    );
    assert_eq!(
        authenticate(Some(&format!("Bearer {token}")), &records, PROFILE),
        Ok(device_id)
    );
    let second = requests.poll(&opened.request_id, &opened.secret, 2001);
    assert_eq!(second, Err(ErrorCode::PairingRefused));
}

#[test]
fn approver_key_is_validated_listed_and_carried_to_the_enrollment() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(2));
    for bad in ["pas-une-cle", ""] {
        assert_eq!(
            requests.open("PC", Some(bad), 0, &mut entropy).err(),
            Some(ErrorCode::InvalidKey)
        );
    }
    let key = approver_key();
    let opened = requests.open("PC approbateur", Some(&key), 0, &mut entropy).unwrap();
    assert!(requests.list(0)[0].approver);
    let target = requests.target(&opened.request_id, 0).unwrap();
    assert_eq!(target.approver_public_key.as_deref(), Some(key.as_str()));
}

#[test]
fn denial_is_handed_once_and_secrets_are_checked() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(3));
    let first = open(&mut requests, "Premier", 0, &mut entropy);
    let refused = Err(ErrorCode::PairingRefused);
    assert_eq!(requests.poll(&first.request_id, "autre-secret", 0), refused);
    assert_eq!(requests.poll("inconnu", &first.secret, 0), refused);
    assert_eq!(requests.poll(&first.request_id, "", 0), refused);
    requests.deny(&first.request_id).unwrap();
    assert_eq!(requests.deny(&first.request_id), Err(ErrorCode::RequestExpired));
    assert_eq!(requests.poll(&first.request_id, &first.secret, 1), Ok(Polled::Denied));
    assert_eq!(requests.poll(&first.request_id, &first.secret, 2), refused);
    assert_eq!(requests.deny("inconnu"), Err(ErrorCode::RequestExpired));
}

#[test]
fn expired_requests_cannot_be_approved_or_polled() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(4));
    let opened = open(&mut requests, "Second", 0, &mut entropy);
    assert_eq!(
        requests.target(&opened.request_id, 300_001).err(),
        Some(ErrorCode::RequestExpired)
    );
    assert_eq!(
        requests.poll(&opened.request_id, &opened.secret, 300_001),
        Err(ErrorCode::PairingRefused)
    );
    let fresh = open(&mut requests, "Troisième", 400_000, &mut entropy);
    let (_, enrolled) = enroll_named("T", Some(PROFILE), None, 0, &[], &mut entropy).unwrap();
    assert_eq!(
        requests.approve(&fresh.request_id, 700_001, "default", &enrolled),
        Err(ErrorCode::RequestExpired)
    );
    assert_eq!(
        requests.approve("inconnu", 0, "default", &enrolled),
        Err(ErrorCode::RequestExpired)
    );
    assert_eq!(requests.list(700_001), Vec::new());
}

#[test]
fn approval_of_a_decided_request_is_refused() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(5));
    let opened = open(&mut requests, "PC", 0, &mut entropy);
    let (_, enrolled) = enroll_named("PC", Some(PROFILE), None, 0, &[], &mut entropy).unwrap();
    requests.approve(&opened.request_id, 0, "v", &enrolled).unwrap();
    assert_eq!(
        requests.approve(&opened.request_id, 0, "v", &enrolled),
        Err(ErrorCode::RequestExpired)
    );
    assert_eq!(
        requests.target(&opened.request_id, 0).err(),
        Some(ErrorCode::RequestExpired)
    );
    assert_eq!(requests.deny(&opened.request_id), Err(ErrorCode::RequestExpired));
}

#[test]
fn names_are_cleaned_and_bounded() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(6));
    for bad in ["", "   ", "\u{1}\u{7f}\t", &"x".repeat(81)] {
        assert_eq!(
            requests.open(bad, None, 0, &mut entropy).err(),
            Some(ErrorCode::InvalidDevice)
        );
    }
    assert!(requests.open(&"x".repeat(80), None, 0, &mut entropy).is_ok());
    assert!(
        requests
            .open(&format!("{}\u{1}", "y".repeat(80)), None, 0, &mut entropy)
            .is_ok()
    );
    assert!(requests.open("\u{7f}é\u{1f} ", None, 0, &mut entropy).is_ok());
    assert_eq!(requests.list(0)[2].name, "é");
}

#[test]
fn pending_requests_are_limited_to_eight() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(7));
    for index in 0..8 {
        open(&mut requests, &format!("Appareil {index}"), 0, &mut entropy);
    }
    assert_eq!(
        requests.open("Trop", None, 0, &mut entropy).err(),
        Some(ErrorCode::PairingLimit)
    );
    assert!(requests.open("Après", None, 300_001, &mut entropy).is_ok());
}

#[test]
fn openings_are_limited_to_ten_per_minute() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(8));
    for index in 0..10 {
        let opened = open(&mut requests, &format!("Appareil {index}"), 1000, &mut entropy);
        requests.deny(&opened.request_id).unwrap();
        requests.poll(&opened.request_id, &opened.secret, 1000).unwrap();
    }
    assert_eq!(
        requests.open("Onzième", None, 1000, &mut entropy).err(),
        Some(ErrorCode::PairingRateLimit)
    );
    assert_eq!(
        requests.open("Onzième", None, 60_999, &mut entropy).err(),
        Some(ErrorCode::PairingRateLimit)
    );
    assert!(requests.open("Après la minute", None, 61_001, &mut entropy).is_ok());
}

#[test]
fn failed_validations_do_not_use_the_rate_limit_budget() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(9));
    for _ in 0..20 {
        assert!(requests.open("", None, 0, &mut entropy).is_err());
    }
    assert!(requests.open("Valide", None, 0, &mut entropy).is_ok());
}

#[test]
fn json_shapes_match_the_protocol() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(10));
    let opened = open(&mut requests, "PC", 0, &mut entropy);
    let shape = serde_json::to_value(&opened).unwrap();
    assert_eq!(shape["expiresInSeconds"], 300);
    assert_eq!(shape.as_object().unwrap().len(), 4);
    let listed = serde_json::to_value(requests.list(0)).unwrap();
    assert_eq!(listed[0]["requestId"], json!(opened.request_id));
    assert_eq!(listed[0].as_object().unwrap().len(), 5);
    let pending = serde_json::to_value(requests.poll(&opened.request_id, &opened.secret, 0).unwrap()).unwrap();
    assert_eq!(pending, json!({"state": "pending", "expiresInSeconds": 300}));
    let denied = serde_json::to_value(Polled::Denied).unwrap();
    assert_eq!(denied, json!({"state": "denied"}));
    let approved = serde_json::to_value(Polled::Approved {
        token: "t".into(),
        device_id: "d".into(),
        vault_id: "v".into(),
    })
    .unwrap();
    assert_eq!(
        approved,
        json!({"state": "approved", "token": "t", "deviceId": "d", "vaultId": "v"})
    );
}

#[test]
fn rounding_follows_math_round() {
    let (mut requests, mut entropy) = (PairRequests::new(), SeededEntropy::new(11));
    let opened = open(&mut requests, "PC", 0, &mut entropy);
    let at = |requests: &mut PairRequests, now: u64| requests.list(now)[0].expires_in_seconds;
    assert_eq!(at(&mut requests, 299_500), 1);
    assert_eq!(at(&mut requests, 299_501), 0);
    assert_eq!(at(&mut requests, 300_000), 0);
    assert_eq!(opened.expires_in_seconds, 300);
}
