//! Enrôlements de Windows Windows Hello, et leurs effets sur les décisions signées.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

mod common;

use common::{Authenticator, World, decision, device, fixture, hex_bytes, message, rsa_private, sign_pss, text};
use rsa::RsaPrivateKey;
use vaultagent_core::{ApprovalPolicy, ErrorCode, HelloEnrollInput};

struct Setup {
    world: World,
    approver: RsaPrivateKey,
    hello: Authenticator,
    replacement: Authenticator,
}

/// Monde avec un demandeur `a`, deux approbateurs `b` et `c` partageant la même clé, et leurs clés factices.
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
        hello: Authenticator::new(&hex_bytes(&text(&data["ec2"]["privateScalarHex"])), 1),
        replacement: Authenticator::new(&[7u8; 32], 2),
    }
}

/// Ouvre l'enrôlement Hello d'un appareil et rend le défi.
fn begin_hello(
    world: &mut World,
    id: &str,
) -> vaultagent_core::Result<(String, Option<vaultagent_core::HelloCredential>)> {
    let (queue, mut env) = world.parts();
    queue
        .begin_hello(id, &mut env)
        .map(|begin| (begin.challenge, begin.hello_credential))
}

/// Corps d'enrôlement Hello signé par l'approbateur, avec la preuve de l'authentificateur.
fn hello_input(s: &Setup, auth: &Authenticator, challenge: &str) -> HelloEnrollInput {
    let message = message("enroll-hello", challenge, true, false);
    HelloEnrollInput {
        credential: Some(auth.credential.clone()),
        hello_proof: Some(auth.proof(&message)),
        signature: Some(sign_pss(&s.approver, &message)),
    }
}

/// Enrôle Hello sur l'appareil `b` avec l'authentificateur principal.
fn enroll_primary(s: &mut Setup) {
    let (challenge, _) = begin_hello(&mut s.world, "b").unwrap();
    let input = hello_input(s, &s.hello, &challenge);
    s.world.enroll_hello("b", input).unwrap();
}

#[test]
fn hello_enrollment_proves_both_keys_persists_the_policy_and_locks_the_session() {
    let mut s = setup();
    let (challenge, existing) = begin_hello(&mut s.world, "b").unwrap();
    assert!(existing.is_none());
    s.world.session.opened = true;
    let mut input = hello_input(&s, &s.hello, &challenge);
    let proof = input.hello_proof.take();
    assert_eq!(s.world.enroll_hello("b", input.clone()), Err(ErrorCode::HelloRequired));
    input.hello_proof = proof;
    let reply = s.world.enroll_hello("b", input).unwrap();
    assert_eq!(reply["helloEnabled"], true);
    assert_eq!(
        (reply["helloRequired"].as_bool(), reply["vaultId"].as_str()),
        (Some(true), Some("default"))
    );
    assert_eq!(reply.as_object().unwrap().len(), 3);
    assert!(!s.world.session.opened);
    assert_eq!(s.world.policy, ApprovalPolicy::requiring_hello("profil-factice"));
    assert_eq!(s.world.records[1].hello_credential.as_ref(), Some(&s.hello.credential));
    assert_eq!(s.world.records[1].hello_enrollment_allowed, Some(false));
    let (queue, env) = s.world.parts();
    assert!(queue.security("b", &env).hello_enabled);
    assert!(queue.security("c", &env).hello_required);
    assert!(queue.security("c", &env).hello_required_for_all);
}

#[test]
fn hello_enrollment_is_refused_for_wrong_context_signature_or_proof() {
    let mut s = setup();
    assert_eq!(
        begin_hello(&mut s.world, "a").err(),
        Some(ErrorCode::HelloEnrollmentRefused)
    );
    assert_eq!(
        begin_hello(&mut s.world, "inconnu").err(),
        Some(ErrorCode::HelloEnrollmentRefused)
    );
    let (challenge, _) = begin_hello(&mut s.world, "b").unwrap();
    let good = hello_input(&s, &s.hello, &challenge);
    let mut bad_signature = good.clone();
    bad_signature.signature = Some(sign_pss(&s.approver, "autre message"));
    assert_eq!(
        s.world.enroll_hello("b", bad_signature),
        Err(ErrorCode::ApprovalRefused)
    );
    let mut no_signature = good.clone();
    no_signature.signature = None;
    assert_eq!(s.world.enroll_hello("b", no_signature), Err(ErrorCode::ApprovalRefused));
    let mut bad_proof = good.clone();
    bad_proof.hello_proof = Some(s.hello.proof("autre message"));
    assert_eq!(s.world.enroll_hello("b", bad_proof), Err(ErrorCode::HelloInvalid));
    assert_eq!(
        s.world.enroll_hello("c", good.clone()),
        Err(ErrorCode::HelloEnrollmentRefused)
    );
    s.world.now = 60_000;
    assert_eq!(s.world.enroll_hello("b", good), Err(ErrorCode::HelloEnrollmentRefused));
}

#[test]
fn hello_enrollment_context_is_single_use() {
    let mut s = setup();
    let (challenge, _) = begin_hello(&mut s.world, "b").unwrap();
    let input = hello_input(&s, &s.hello, &challenge);
    s.world.enroll_hello("b", input.clone()).unwrap();
    assert_eq!(s.world.enroll_hello("b", input), Err(ErrorCode::HelloEnrollmentRefused));
}

#[test]
fn once_hello_is_required_other_devices_need_an_authorization_and_the_credential_is_fixed() {
    let mut s = setup();
    let (early, _) = begin_hello(&mut s.world, "c").unwrap();
    s.world.records[2].hello_enrollment_allowed = Some(false);
    enroll_primary(&mut s);
    assert_eq!(
        begin_hello(&mut s.world, "c").err(),
        Some(ErrorCode::HelloEnrollmentAuthorizationRequired)
    );
    let late = hello_input(&s, &s.replacement, &early);
    assert_eq!(
        s.world.enroll_hello("c", late),
        Err(ErrorCode::HelloEnrollmentAuthorizationRequired)
    );
    let (challenge, existing) = begin_hello(&mut s.world, "b").unwrap();
    assert_eq!(existing.as_ref(), Some(&s.hello.credential));
    let replacement = hello_input(&s, &s.replacement, &challenge);
    assert_eq!(
        s.world.enroll_hello("b", replacement),
        Err(ErrorCode::HelloEnrollmentRefused)
    );
    let same = hello_input(&s, &s.hello, &challenge);
    let before = s.world.records.clone();
    s.world.session.opened = true;
    assert!(s.world.enroll_hello("b", same).is_ok());
    assert_eq!(s.world.records, before);
    assert!(!s.world.session.opened);
}

#[test]
fn an_authorized_new_device_may_enroll_when_hello_is_required_for_all() {
    let mut s = setup();
    enroll_primary(&mut s);
    s.world.records[2].hello_enrollment_allowed = Some(true);
    let (challenge, _) = begin_hello(&mut s.world, "c").unwrap();
    let input = hello_input(&s, &s.replacement, &challenge);
    s.world.enroll_hello("c", input).unwrap();
    assert_eq!(
        s.world.records[2].hello_credential.as_ref(),
        Some(&s.replacement.credential)
    );
    assert_eq!(s.world.records[2].hello_enrollment_allowed, Some(false));
}

#[test]
fn approval_requires_the_hello_proof_of_the_approver_once_hello_is_enabled() {
    let mut s = setup();
    enroll_primary(&mut s);
    let pending = s.world.pending("a");
    let challenge = s.world.challenge();
    let input = decision(&s.approver, &pending.request_id, &challenge, true, false);
    let proof = s.hello.proof(&message(&pending.request_id, &challenge, true, false));
    assert_eq!(s.world.signed("b", input.clone()), Err(ErrorCode::HelloRequired));
    let mut other_message = input.clone();
    other_message.hello_proof = Some(s.hello.proof("autre message"));
    assert_eq!(s.world.signed("b", other_message), Err(ErrorCode::HelloInvalid));
    let mut without_credential = input.clone();
    without_credential.hello_proof = Some(proof.clone());
    assert_eq!(s.world.signed("c", without_credential), Err(ErrorCode::HelloInvalid));
    assert_eq!(s.world.session.unlocks, 0);
    let mut proven = input;
    proven.hello_proof = Some(proof);
    let reply = s.world.signed("b", proven.clone()).unwrap();
    assert_eq!(reply["state"], "approved");
    assert_eq!(s.world.session.unlocks, 1);
    assert_eq!(s.world.signed("b", proven), Err(ErrorCode::RequestExpired));
}

#[test]
fn a_refusal_and_the_reset_do_not_need_the_hello_proof() {
    let mut s = setup();
    enroll_primary(&mut s);
    let pending = s.world.pending("a");
    let challenge = s.world.challenge();
    let denied = s
        .world
        .signed(
            "b",
            decision(&s.approver, &pending.request_id, &challenge, false, false),
        )
        .unwrap();
    assert_eq!(denied["state"], "denied");
    let context = {
        let (queue, mut env) = s.world.parts();
        queue.context("b", &mut env).unwrap()
    };
    let reset = s
        .world
        .signed(
            "b",
            decision(&s.approver, "reset-auto", &context.challenge, false, false),
        )
        .unwrap();
    assert_eq!(reset["automaticAcceptance"], false);
}
