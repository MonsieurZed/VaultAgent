//! Appareils : codes d'association, enrôlement, authentification, liste publique, révocation, acceptation
//! automatique, enrôlements Hello et élévation, avec horloge et aléa déterministes.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::Value;
use vaultagent_core::device_list::{
    authenticate, authorize_hello_enrollment, clear_automatic, enroll_elevation, enroll_hello, list, revoke,
    set_automatic,
};
use vaultagent_core::device_record::iso_from_millis;
use vaultagent_core::devices::{enroll, enroll_named};
use vaultagent_core::{DeviceRecord, Devices, ErrorCode, HelloCredential, SeededEntropy, digest};

const PROFILE: &str = "profil-factice";
const KEY: &str = "cle-approbateur-factice";

/// En-tête `Authorization` d'un jeton.
fn header(token: &str) -> String {
    format!("Bearer {token}")
}

/// Enrôle un appareil de plus et rend la liste, le jeton et l'identifiant.
fn add(
    records: &[DeviceRecord],
    entropy: &mut SeededEntropy,
    approver: Option<&str>,
) -> (Vec<DeviceRecord>, String, String) {
    let (next, enrolled) = enroll("PC", PROFILE, approver, 1000, records, entropy).unwrap();
    (next, enrolled.token.to_string(), enrolled.device_id)
}

/// Justificatif Hello factice.
fn credential() -> HelloCredential {
    HelloCredential {
        id: "cred".into(),
        key_type: "EC2".into(),
        first: "x".into(),
        second: "y".into(),
    }
}

#[test]
fn iso_dates_match_node_to_iso_string() {
    let data: Value = serde_json::from_str(include_str!("fixtures/crypto.json")).unwrap();
    for case in data["dates"].as_array().unwrap() {
        assert_eq!(
            iso_from_millis(case["millis"].as_u64().unwrap()),
            case["iso"].as_str().unwrap()
        );
    }
}

#[test]
fn issued_code_pairs_once_and_creates_an_authenticating_device() {
    let mut entropy = SeededEntropy::new(1);
    let mut devices = Devices::new();
    let issued = devices.issue("  Agent Linux  ", PROFILE, 0, &mut entropy).unwrap();
    assert_eq!((issued.code.len(), issued.expires_in_seconds), (24, 300));
    let (records, enrolled) = devices
        .pair(Some(&issued.code), PROFILE, 1000, &[], &mut entropy)
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "Agent Linux");
    assert_eq!(records[0].created, "1970-01-01T00:00:01.000Z");
    assert_eq!(records[0].hash, digest(enrolled.token.as_bytes()));
    assert_eq!(
        authenticate(Some(&header(&enrolled.token)), &records, PROFILE),
        Ok(enrolled.device_id.clone())
    );
    let again = devices.pair(Some(&issued.code), PROFILE, 1001, &records, &mut entropy);
    assert_eq!(again.err(), Some(ErrorCode::PairingRefused));
}

#[test]
fn pairing_refuses_unknown_expired_and_foreign_profile_codes() {
    let mut entropy = SeededEntropy::new(2);
    let mut devices = Devices::new();
    let expired = devices.issue("A", PROFILE, 0, &mut entropy).unwrap();
    let foreign = devices.issue("B", "autre-profil", 0, &mut entropy).unwrap();
    let valid = devices.issue("C", PROFILE, 0, &mut entropy).unwrap();
    let attempts = [
        (None, 10),
        (Some("inconnu"), 10),
        (Some(expired.code.as_str()), 300_001),
        (Some(foreign.code.as_str()), 10),
    ];
    for (code, now) in attempts {
        let result = devices.pair(code, PROFILE, now, &[], &mut entropy);
        assert_eq!(result.err(), Some(ErrorCode::PairingRefused));
    }
    assert!(
        devices
            .pair(Some(&valid.code), PROFILE, 300_000, &[], &mut entropy)
            .is_ok()
    );
}

#[test]
fn issue_validates_names_and_limits_pending_codes() {
    let mut entropy = SeededEntropy::new(3);
    let mut devices = Devices::new();
    for name in ["", "   ", "\u{a0}\t", &"x".repeat(81)] {
        assert_eq!(
            devices.issue(name, PROFILE, 0, &mut entropy).err(),
            Some(ErrorCode::InvalidDevice)
        );
    }
    assert!(devices.issue(&"é".repeat(80), PROFILE, 0, &mut entropy).is_ok());
    for index in 0..7 {
        devices
            .issue(&format!("Appareil {index}"), PROFILE, 0, &mut entropy)
            .unwrap();
    }
    assert_eq!(
        devices.issue("Trop", PROFILE, 0, &mut entropy).err(),
        Some(ErrorCode::PairingLimit)
    );
    assert!(
        devices
            .issue("Après expiration", PROFILE, 300_001, &mut entropy)
            .is_ok()
    );
}

#[test]
fn pairing_attempts_are_limited_to_ten_per_minute() {
    let mut entropy = SeededEntropy::new(4);
    let mut devices = Devices::new();
    for _ in 0..10 {
        assert_eq!(
            devices.pair(Some("x"), PROFILE, 5000, &[], &mut entropy).err(),
            Some(ErrorCode::PairingRefused)
        );
    }
    assert_eq!(
        devices.pair(Some("x"), PROFILE, 5000, &[], &mut entropy).err(),
        Some(ErrorCode::PairingRateLimit)
    );
    assert_eq!(
        devices.pair(Some("x"), PROFILE, 64_999, &[], &mut entropy).err(),
        Some(ErrorCode::PairingRateLimit)
    );
    assert_eq!(
        devices.pair(Some("x"), PROFILE, 65_001, &[], &mut entropy).err(),
        Some(ErrorCode::PairingRefused)
    );
}

#[test]
fn enrollment_stops_at_32_devices_and_drops_other_profiles() {
    let mut entropy = SeededEntropy::new(5);
    let mut records = Vec::new();
    for _ in 0..32 {
        records = add(&records, &mut entropy, None).0;
    }
    let full = enroll("PC", PROFILE, None, 0, &records, &mut entropy);
    assert_eq!(full.err(), Some(ErrorCode::DeviceLimit));
    let mut mixed = vec![records[0].clone()];
    mixed[0].profile = "ancien".into();
    mixed.push(records[1].clone());
    let (next, _) = enroll("Neuf", PROFILE, None, 0, &mixed, &mut entropy).unwrap();
    assert_eq!(next.len(), 2);
    assert!(next.iter().all(|device| device.profile == PROFILE));
}

#[test]
fn named_enrollment_needs_a_profile_and_grants_hello_with_an_approver_key() {
    let mut entropy = SeededEntropy::new(6);
    let missing = enroll_named("PC", None, None, 0, &[], &mut entropy);
    assert_eq!(missing.err(), Some(ErrorCode::NotConfigured));
    let (plain, _) = enroll_named("PC", Some(PROFILE), None, 0, &[], &mut entropy).unwrap();
    assert!(plain[0].approver_public_key.is_none() && plain[0].hello_enrollment_allowed.is_none());
    let (approver, _) = enroll_named("PC", Some(PROFILE), Some(KEY), 0, &[], &mut entropy).unwrap();
    assert_eq!(approver[0].approver_public_key.as_deref(), Some(KEY));
    assert_eq!(approver[0].hello_enrollment_allowed, Some(true));
    let json = serde_json::to_value(&plain[0]).unwrap();
    assert!(json.get("approverPublicKey").is_none() && json.get("autoUnlock").is_none());
}

#[test]
fn authenticate_refuses_every_bad_token() {
    let mut entropy = SeededEntropy::new(7);
    let (records, token, id) = add(&[], &mut entropy, None);
    assert_eq!(authenticate(Some(&header(&token)), &records, PROFILE), Ok(id.clone()));
    let other_secret = format!("{}.{}", token.rsplit_once('.').unwrap().0, "B".repeat(43));
    for bad in [
        None,
        Some(String::new()),
        Some(token.clone()),
        Some(header(&other_secret)),
        Some(header("va1.x.y")),
    ] {
        assert_eq!(
            authenticate(bad.as_deref(), &records, PROFILE),
            Err(ErrorCode::Unauthorized)
        );
    }
    assert_eq!(
        authenticate(Some(&header(&token)), &records, "autre"),
        Err(ErrorCode::Unauthorized)
    );
    assert_eq!(
        authenticate(Some(&header(&token)), &[], PROFILE),
        Err(ErrorCode::Unauthorized)
    );
}

#[test]
fn list_is_public_and_filtered_by_profile_then_revoke_removes() {
    let mut entropy = SeededEntropy::new(8);
    let (records, _, id) = add(&[], &mut entropy, Some(KEY));
    let (mut records, _, second) = add(&records, &mut entropy, None);
    records[1].profile = "autre".into();
    let shown = list(&records, PROFILE);
    assert_eq!(shown.len(), 1);
    let json = serde_json::to_value(&shown[0]).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 4);
    assert_eq!(json["autoUnlock"], false);
    assert_eq!(revoke(&records, &id).len(), 1);
    assert_eq!(revoke(&records, &second)[0].id, id);
    assert_eq!(revoke(&records, "inconnu").len(), 2);
}

#[test]
fn automatic_unlock_can_be_set_and_cleared() {
    let mut entropy = SeededEntropy::new(9);
    let (records, _, id) = add(&[], &mut entropy, None);
    let on = set_automatic(&records, &id, PROFILE, true).unwrap();
    assert!(list(&on, PROFILE)[0].auto_unlock);
    assert_eq!(
        set_automatic(&on, &id, PROFILE, false).unwrap()[0].auto_unlock,
        Some(false)
    );
    assert_eq!(
        set_automatic(&records, "inconnu", PROFILE, true).err(),
        Some(ErrorCode::InvalidDevice)
    );
    assert_eq!(
        set_automatic(&records, &id, "autre", true).err(),
        Some(ErrorCode::InvalidDevice)
    );
    assert!(
        clear_automatic(&on)
            .iter()
            .all(|device| device.auto_unlock == Some(false))
    );
}

#[test]
fn hello_enrollment_follows_the_authorization_flow() {
    let mut entropy = SeededEntropy::new(10);
    let (records, _, id) = add(&[], &mut entropy, Some(KEY));
    let (plain, _, plain_id) = add(&records, &mut entropy, None);
    let refused = Some(ErrorCode::HelloEnrollmentRefused);
    assert_eq!(authorize_hello_enrollment(&plain, &plain_id, PROFILE).err(), refused);
    assert_eq!(authorize_hello_enrollment(&plain, "inconnu", PROFILE).err(), refused);
    assert_eq!(authorize_hello_enrollment(&plain, &id, "autre").err(), refused);
    assert_eq!(enroll_hello(&plain, &plain_id, PROFILE, credential()).err(), refused);
    let enrolled = enroll_hello(&plain, &id, PROFILE, credential()).unwrap();
    assert_eq!(enrolled[0].hello_enrollment_allowed, Some(false));
    assert_eq!(enrolled[0].hello_credential, Some(credential()));
    assert_eq!(enroll_hello(&enrolled, &id, PROFILE, credential()).err(), refused);
    let reopened = authorize_hello_enrollment(&enrolled, &id, PROFILE).unwrap();
    assert_eq!(reopened[0].hello_enrollment_allowed, Some(true));
    let json = serde_json::to_value(&enrolled[0]).unwrap();
    assert_eq!(json["helloCredential"]["keyType"], "EC2");
}

#[test]
fn elevation_enrollment_happens_once_and_needs_an_approver_key() {
    let mut entropy = SeededEntropy::new(11);
    let (records, _, id) = add(&[], &mut entropy, Some(KEY));
    let (plain, _, plain_id) = add(&records, &mut entropy, None);
    let refused = Some(ErrorCode::ElevationEnrollmentRefused);
    assert_eq!(enroll_elevation(&plain, &plain_id, PROFILE, "k").err(), refused);
    assert_eq!(enroll_elevation(&plain, "inconnu", PROFILE, "k").err(), refused);
    assert_eq!(enroll_elevation(&plain, &id, "autre", "k").err(), refused);
    let done = enroll_elevation(&plain, &id, PROFILE, "cle-elevation").unwrap();
    assert_eq!(done[0].elevation_public_key.as_deref(), Some("cle-elevation"));
    assert_eq!(enroll_elevation(&done, &id, PROFILE, "autre").err(), refused);
}
