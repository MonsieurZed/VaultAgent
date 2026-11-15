//! Enrôlement de la clé d'élévation et contresignature obligatoire des accords.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

mod common;

use common::{World, decision, device, fixture, message, rsa_private, sign_pss, text};
use rsa::RsaPrivateKey;
use serde_json::Value;
use vaultagent_core::{ElevationEnrollInput, ErrorCode, SessionControl};

struct Setup {
    world: World,
    approver: RsaPrivateKey,
    elevation: RsaPrivateKey,
    elevation_public: String,
}

/// Monde avec un demandeur `a`, deux approbateurs `b` et `c`, et les clés factices d'approbation et d'élévation.
fn setup() -> Setup {
    let data = fixture();
    let key = text(&data["approver"]["publicKey"]);
    let world = World::new(vec![
        device("a", "Agent factice", None),
        device("b", "PC approbateur", Some(&key)),
        device("c", "Second PC", Some(&key)),
    ]);
    Setup {
        world,
        approver: rsa_private(&text(&data["approver"]["privateKeyPkcs8"])),
        elevation: rsa_private(&text(&data["elevation"]["privateKeyPkcs8"])),
        elevation_public: text(&data["elevation"]["publicKey"]),
    }
}

#[test]
fn elevation_enrollment_needs_both_signatures_and_a_valid_key() {
    let mut s = setup();
    let begin = |w: &mut World, id: &str| {
        let (queue, mut env) = w.parts();
        queue.begin_elevation(id, &mut env).map(|reply| reply.challenge)
    };
    assert_eq!(begin(&mut s.world, "a"), Err(ErrorCode::ElevationEnrollmentRefused));
    let challenge = begin(&mut s.world, "b").unwrap();
    let text = message("enroll-elevation", &challenge, true, false);
    let input = ElevationEnrollInput {
        public_key: s.elevation_public.clone(),
        signature: Some(sign_pss(&s.approver, &text)),
        elevation_signature: Some(sign_pss(&s.elevation, &text)),
    };
    let tweak = |change: &dyn Fn(&mut ElevationEnrollInput)| {
        let mut copy = input.clone();
        change(&mut copy);
        copy
    };
    let forged = sign_pss(&s.elevation, "autre message");
    assert_eq!(
        s.world
            .enroll_elevation("b", tweak(&|i| i.signature = Some(forged.clone()))),
        Err(ErrorCode::ApprovalRefused)
    );
    assert_eq!(
        s.world
            .enroll_elevation("b", tweak(&|i| i.elevation_signature = Some(forged.clone()))),
        Err(ErrorCode::ElevationRequired)
    );
    assert_eq!(
        s.world.enroll_elevation("b", tweak(&|i| i.elevation_signature = None)),
        Err(ErrorCode::ElevationRequired)
    );
    assert_eq!(
        s.world
            .enroll_elevation("b", tweak(&|i| i.public_key = "pas-une-cle".to_owned())),
        Err(ErrorCode::InvalidKey)
    );
    assert_eq!(
        s.world.enroll_elevation("c", input.clone()),
        Err(ErrorCode::ElevationEnrollmentRefused)
    );
    let reply = s.world.enroll_elevation("b", input.clone()).unwrap();
    assert_eq!(
        (reply["elevationEnabled"].as_bool(), reply["vaultId"].as_str()),
        (Some(true), Some("default"))
    );
    assert_eq!(
        s.world.records[1].elevation_public_key.as_deref(),
        Some(s.elevation_public.as_str())
    );
    assert_eq!(
        s.world.enroll_elevation("b", input),
        Err(ErrorCode::ElevationEnrollmentRefused)
    );
    assert_eq!(begin(&mut s.world, "b"), Err(ErrorCode::ElevationEnrollmentRefused));
}

#[test]
fn an_enrolled_elevation_key_must_countersign_every_approval() {
    let mut s = setup();
    s.world.records[1].elevation_public_key = Some(s.elevation_public.clone());
    let pending = s.world.pending("a");
    let challenge = s.world.challenge();
    let input = decision(&s.approver, &pending.request_id, &challenge, true, false);
    let text = message(&pending.request_id, &challenge, true, false);
    assert_eq!(s.world.signed("b", input.clone()), Err(ErrorCode::ElevationRequired));
    let mut wrong = input.clone();
    wrong.elevation_signature = Some(sign_pss(&s.approver, &text));
    assert_eq!(s.world.signed("b", wrong), Err(ErrorCode::ElevationRequired));
    let (queue, env) = s.world.parts();
    assert!(queue.security("b", &env).elevation_enabled);
    assert_eq!(s.world.session.unlocks, 0);
    let mut countersigned = input;
    countersigned.elevation_signature = Some(sign_pss(&s.elevation, &text));
    let reply: Value = s.world.signed("b", countersigned).unwrap();
    assert_eq!(reply["state"], "approved");
    assert!(s.world.session.opened);
    s.world.session.lock();
    let second = s.world.pending("a");
    let nonce = s.world.challenge();
    let denied = s
        .world
        .signed("b", decision(&s.approver, &second.request_id, &nonce, false, false))
        .unwrap();
    assert_eq!(denied["state"], "denied");
}
