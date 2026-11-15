//! Décisions signées par un PC approbateur : RSA-PSS, accord mémorisé, retrait de l'acceptation automatique,
//! contexte `reset-auto`, sécurité d'approbation et vue `status`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

mod common;

use common::{World, decision, device, fixture, rsa_private, text};
use serde_json::to_value;
use vaultagent_core::{ErrorCode, RequestState, SignedDecisionInput};

struct Keys {
    approver: rsa::RsaPrivateKey,
    other: rsa::RsaPrivateKey,
}

/// Monde avec un demandeur `a`, un approbateur `b` et un appareil `c` sans clé.
fn setup() -> (World, Keys) {
    let data = fixture();
    let keys = Keys {
        approver: rsa_private(&text(&data["approver"]["privateKeyPkcs8"])),
        other: rsa_private(&text(&data["other"]["privateKeyPkcs8"])),
    };
    let world = World::new(vec![
        device("a", "Agent factice", None),
        device("b", "PC approbateur", Some(&text(&data["approver"]["publicKey"]))),
        device("c", "Sans clé", None),
    ]);
    (world, keys)
}

/// Rejoue une décision signée avec un corps modifié.
fn tampered(input: &SignedDecisionInput, change: impl Fn(&mut SignedDecisionInput)) -> SignedDecisionInput {
    let mut copy = input.clone();
    change(&mut copy);
    copy
}

#[test]
fn remembered_approval_unlocks_enables_automatic_acceptance_and_cannot_be_replayed() {
    let (mut w, keys) = setup();
    let pending = w.pending("a");
    let challenge = w.challenge();
    let input = decision(&keys.approver, &pending.request_id, &challenge, true, true);
    assert_eq!(w.signed("c", input.clone()), Err(ErrorCode::ApproverRequired));
    assert_eq!(
        w.signed("b", tampered(&input, |i| i.approve = Some(false))),
        Err(ErrorCode::ApprovalRefused)
    );
    assert_eq!(
        w.signed("b", tampered(&input, |i| i.remember = Some(false))),
        Err(ErrorCode::ApprovalRefused)
    );
    assert_eq!(w.session.unlocks, 0);
    let reply = w.signed("b", input.clone()).unwrap();
    assert_eq!(reply["state"], "approved");
    assert_eq!(w.session.unlocks, 1);
    assert_eq!(w.records[0].auto_unlock, Some(true));
    assert_eq!(w.records[1].auto_unlock, None);
    assert_eq!(w.signed("b", input), Err(ErrorCode::RequestExpired));
}

#[test]
fn signature_from_another_key_or_another_vault_is_refused() {
    let (mut w, keys) = setup();
    let pending = w.pending("a");
    let challenge = w.challenge();
    let forged = decision(&keys.other, &pending.request_id, &challenge, true, false);
    assert_eq!(w.signed("b", forged), Err(ErrorCode::ApprovalRefused));
    let foreign = vaultagent_core::decision_message(
        common::ORIGIN,
        "autre-coffre",
        &pending.request_id,
        &challenge,
        true,
        false,
    );
    let mut input = decision(&keys.approver, &pending.request_id, &challenge, true, false);
    input.signature = Some(common::sign_pss(&keys.approver, &foreign));
    assert_eq!(w.signed("b", input), Err(ErrorCode::ApprovalRefused));
    assert_eq!(w.session.unlocks, 0);
}

#[test]
fn incomplete_bodies_and_expired_requests_are_refused() {
    let (mut w, keys) = setup();
    let pending = w.pending("a");
    let challenge = w.challenge();
    let input = decision(&keys.approver, &pending.request_id, &challenge, true, false);
    for broken in [
        tampered(&input, |i| i.approve = None),
        tampered(&input, |i| i.remember = None),
        tampered(&input, |i| i.signature = None),
    ] {
        assert_eq!(w.signed("b", broken), Err(ErrorCode::ApproverRequired));
    }
    assert_eq!(
        w.signed("b", tampered(&input, |i| i.request_id = "inconnue".to_owned())),
        Err(ErrorCode::RequestExpired)
    );
    w.now = 300_000;
    assert_eq!(w.signed("b", input), Err(ErrorCode::RequestExpired));
}

#[test]
fn a_signed_refusal_denies_without_unlocking_or_remembering() {
    let (mut w, keys) = setup();
    let pending = w.pending("a");
    let challenge = w.challenge();
    let reply = w
        .signed(
            "b",
            decision(&keys.approver, &pending.request_id, &challenge, false, true),
        )
        .unwrap();
    assert_eq!(reply["state"], "denied");
    assert_eq!(w.session.unlocks, 0);
    assert_eq!(w.records[0].auto_unlock, None);
    assert_eq!(w.current("a").unwrap().state, RequestState::Denied);
}

#[test]
fn reset_context_removes_every_automatic_acceptance_once() {
    let (mut w, keys) = setup();
    for record in &mut w.records {
        record.auto_unlock = Some(true);
    }
    assert_eq!(
        {
            let (queue, mut env) = w.parts();
            queue.context("c", &mut env).err()
        },
        Some(ErrorCode::ApproverRequired)
    );
    let context = {
        let (queue, mut env) = w.parts();
        queue.context("b", &mut env).unwrap()
    };
    let value = to_value(&context).unwrap();
    assert_eq!(
        (value["requestId"].as_str(), value["vaultId"].as_str()),
        (Some("reset-auto"), Some("default"))
    );
    let input = decision(&keys.approver, "reset-auto", &context.challenge, false, false);
    assert_eq!(
        w.signed("b", tampered(&input, |i| i.remember = Some(true))),
        Err(ErrorCode::ApprovalRefused)
    );
    let reply = w.signed("b", input.clone()).unwrap();
    assert_eq!(
        reply,
        to_value(vaultagent_core::unlock_replies::AutomaticReset {
            automatic_acceptance: false
        })
        .unwrap()
    );
    assert!(w.records.iter().all(|record| record.auto_unlock == Some(false)));
    assert_eq!(w.signed("b", input), Err(ErrorCode::RequestExpired));
    assert_eq!(w.pending("a").state, RequestState::Pending);
}

#[test]
fn reset_context_expires_after_sixty_seconds() {
    let (mut w, keys) = setup();
    let context = {
        let (queue, mut env) = w.parts();
        queue.context("b", &mut env).unwrap()
    };
    w.now = 60_000;
    assert_eq!(
        w.signed(
            "b",
            decision(&keys.approver, "reset-auto", &context.challenge, false, false)
        ),
        Err(ErrorCode::RequestExpired)
    );
}

#[test]
fn status_shows_the_approver_its_pending_requests_and_the_client_its_own() {
    let (mut w, _) = setup();
    let pending = w.pending("a");
    let (queue, mut env) = w.parts();
    let approver = to_value(queue.status("b", "Coffre", &mut env)).unwrap();
    assert_eq!(approver["approvalSecurity"]["registered"], true);
    assert_eq!(approver["approvalSecurity"]["helloEnrollmentAllowed"], true);
    assert_eq!(approver["approvalSecurity"]["helloRequired"], false);
    assert_eq!(approver["unlockRequest"], serde_json::Value::Null);
    let listed = &approver["approvalRequests"][0];
    assert_eq!(listed["deviceName"], "Agent factice");
    assert_eq!(listed["vaultName"], "Coffre");
    assert_eq!(listed["helloRequired"], false);
    assert_eq!(listed["elevationRequired"], false);
    assert_eq!(listed["helloCredential"], serde_json::Value::Null);
    let client = to_value(queue.status("a", "Coffre", &mut env)).unwrap();
    assert_eq!(client["approvalRequests"].as_array().unwrap().len(), 0);
    assert_eq!(client["unlockRequest"]["requestId"], listed["requestId"]);
    assert_eq!(client["approvalSecurity"]["registered"], false);
    assert_eq!(pending.request_id, listed["requestId"].as_str().unwrap());
}

#[test]
fn security_of_an_unknown_device_is_all_false() {
    let (mut w, _) = setup();
    let (queue, env) = w.parts();
    let value = to_value(queue.security("inconnu", &env)).unwrap();
    assert!(value.as_object().unwrap().values().all(|flag| flag == false));
    assert_eq!(value.as_object().unwrap().len(), 6);
}
