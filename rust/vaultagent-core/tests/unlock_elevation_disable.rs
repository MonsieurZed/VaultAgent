//! Retrait de la clé d'élévation sur preuve signée par cette clé : succès et refus pour chaque cause.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

mod common;

use common::{World, device, fixture, message, rsa_private, sign_pss, text};
use rsa::RsaPrivateKey;
use vaultagent_core::{ElevationDisableInput, ErrorCode};

struct Setup {
    world: World,
    approver: RsaPrivateKey,
    elevation: RsaPrivateKey,
    other: RsaPrivateKey,
}

/// Monde avec un demandeur `a`, un approbateur `b` déjà enrôlé en élévation et un approbateur `c` sans élévation.
fn setup() -> Setup {
    let data = fixture();
    let key = text(&data["approver"]["publicKey"]);
    let mut world = World::new(vec![
        device("a", "Agent factice", None),
        device("b", "PC approbateur", Some(&key)),
        device("c", "Second PC", Some(&key)),
    ]);
    world.records[1].elevation_public_key = Some(text(&data["elevation"]["publicKey"]));
    Setup {
        world,
        approver: rsa_private(&text(&data["approver"]["privateKeyPkcs8"])),
        elevation: rsa_private(&text(&data["elevation"]["privateKeyPkcs8"])),
        other: rsa_private(&text(&data["other"]["privateKeyPkcs8"])),
    }
}

/// Ouvre le contexte de retrait d'un appareil.
fn begin(world: &mut World, id: &str) -> Result<String, ErrorCode> {
    let (queue, mut env) = world.parts();
    queue.begin_elevation_disable(id, &mut env).map(|reply| {
        assert_eq!(reply.request_id, "disable-elevation");
        reply.challenge
    })
}

/// Termine le retrait d'un appareil, mutations appliquées.
fn finish(world: &mut World, id: &str, input: &ElevationDisableInput) -> Result<serde_json::Value, ErrorCode> {
    let (queue, mut env) = world.parts();
    let outcome = queue.complete_elevation_disable(id, input, common::ORIGIN, &mut env)?;
    Ok(serde_json::to_value(world.apply(outcome)).unwrap())
}

/// Indicateur `elevationRequired` de chaque demande en attente vue par l'approbateur `b`.
fn required(world: &mut World) -> Vec<bool> {
    let (queue, mut env) = world.parts();
    let status = queue.status("b", "Coffre", &mut env);
    status.approval_requests.iter().map(|r| r.elevation_required).collect()
}

/// Corps de retrait signé par les deux clés données.
fn proof(approver: &RsaPrivateKey, elevation: &RsaPrivateKey, challenge: &str) -> ElevationDisableInput {
    let text = message("disable-elevation", challenge, true, false);
    ElevationDisableInput {
        signature: Some(sign_pss(approver, &text)),
        elevation_signature: Some(sign_pss(elevation, &text)),
    }
}

#[test]
fn disable_is_accepted_and_clears_the_key_and_the_requirement() {
    let mut s = setup();
    s.world.pending("a");
    assert_eq!(required(&mut s.world), vec![true]);
    let challenge = begin(&mut s.world, "b").unwrap();
    let reply = finish(&mut s.world, "b", &proof(&s.approver, &s.elevation, &challenge)).unwrap();
    assert_eq!(
        (reply["elevationEnabled"].as_bool(), reply["vaultId"].as_str()),
        (Some(false), Some("default"))
    );
    assert_eq!(s.world.records[1].elevation_public_key, None);
    let (queue, env) = s.world.parts();
    assert!(!queue.security("b", &env).elevation_enabled);
    assert_eq!(required(&mut s.world), vec![false]);
}

#[test]
fn disable_is_refused_without_an_approver_or_an_enrolled_key() {
    let mut s = setup();
    assert_eq!(begin(&mut s.world, "a"), Err(ErrorCode::ApproverRequired));
    assert_eq!(begin(&mut s.world, "c"), Err(ErrorCode::ElevationNotEnrolled));
    let input = proof(&s.approver, &s.elevation, "défi");
    assert_eq!(finish(&mut s.world, "a", &input), Err(ErrorCode::ApproverRequired));
    assert_eq!(finish(&mut s.world, "c", &input), Err(ErrorCode::ElevationNotEnrolled));
}

#[test]
fn disable_is_refused_for_a_missing_expired_or_reused_context() {
    let mut s = setup();
    let input = proof(&s.approver, &s.elevation, "défi");
    assert_eq!(finish(&mut s.world, "b", &input), Err(ErrorCode::RequestExpired));
    let challenge = begin(&mut s.world, "b").unwrap();
    s.world.now += 61_000;
    let late = proof(&s.approver, &s.elevation, &challenge);
    assert_eq!(finish(&mut s.world, "b", &late), Err(ErrorCode::RequestExpired));
    let fresh = begin(&mut s.world, "b").unwrap();
    finish(&mut s.world, "b", &proof(&s.approver, &s.elevation, &fresh)).unwrap();
    let key = text(&fixture()["elevation"]["publicKey"]);
    s.world.records[1].elevation_public_key = Some(key);
    assert_eq!(
        finish(&mut s.world, "b", &proof(&s.approver, &s.elevation, &fresh)),
        Err(ErrorCode::RequestExpired)
    );
}

#[test]
fn disable_is_refused_for_bad_signatures_and_keeps_the_key() {
    let mut s = setup();
    let challenge = begin(&mut s.world, "b").unwrap();
    let forged = sign_pss(&s.elevation, "autre message");
    let mut bad_approver = proof(&s.approver, &s.elevation, &challenge);
    bad_approver.signature = Some(forged.clone());
    assert_eq!(
        finish(&mut s.world, "b", &bad_approver),
        Err(ErrorCode::ApprovalRefused)
    );
    let mut bad_elevation = proof(&s.approver, &s.elevation, &challenge);
    bad_elevation.elevation_signature = Some(forged);
    assert_eq!(
        finish(&mut s.world, "b", &bad_elevation),
        Err(ErrorCode::ApprovalRefused)
    );
    let mut missing = proof(&s.approver, &s.elevation, &challenge);
    missing.elevation_signature = None;
    assert_eq!(finish(&mut s.world, "b", &missing), Err(ErrorCode::ApprovalRefused));
    let other_key = proof(&s.approver, &s.other, &challenge);
    assert_eq!(finish(&mut s.world, "b", &other_key), Err(ErrorCode::ApprovalRefused));
    assert!(s.world.records[1].elevation_public_key.is_some());
}
