//! Demandes de déverrouillage : création, déduplication, décision de l'administrateur, expiration, invalidation,
//! acceptation automatique, liste et formes JSON publiques.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

mod common;

use common::{World, device};
use serde_json::{Value, to_value};
use vaultagent_core::{ErrorCode, RequestState, SessionControl, UnlockReply};

/// Monde avec deux appareils sans clé d'approbateur.
fn world() -> World {
    World::new(vec![device("a", "PC factice", None), device("b", "Autre PC", None)])
}

/// Passe l'appareil donné en acceptation automatique.
fn automatic(world: &mut World, id: &str) {
    for record in &mut world.records {
        if record.id == id {
            record.auto_unlock = Some(true);
        }
    }
}

#[test]
fn request_is_deduplicated_and_only_an_explicit_decision_unlocks() {
    let mut w = world();
    let pending = w.pending("a");
    assert_eq!(pending.state, RequestState::Pending);
    assert_eq!(w.session.unlocks, 0);
    assert_eq!(w.pending("a"), pending);
    assert!(w.current("b").is_none());
    assert_eq!(w.current("a").unwrap(), pending);
    let approved = w.decide(&pending.request_id, true, None).unwrap();
    assert_eq!(approved.state, RequestState::Approved);
    assert_eq!(w.current("a").unwrap().state, RequestState::Approved);
    assert_eq!(w.session.unlocks, 1);
    assert_eq!(
        w.decide(&pending.request_id, true, None),
        Err(ErrorCode::RequestExpired)
    );
}

#[test]
fn already_unlocked_vault_answers_without_creating_a_request() {
    let mut w = world();
    let pending = w.pending("a");
    w.decide(&pending.request_id, true, None).unwrap();
    let reply = w.request("b").unwrap();
    assert_eq!(
        to_value(&reply).unwrap(),
        serde_json::from_str::<Value>(r#"{"state":"unlocked","unlocked":true}"#).unwrap()
    );
    assert!(w.current("b").is_none());
    assert_eq!(w.session.unlocks, 1);
    let (queue, mut env) = w.parts();
    assert_eq!(queue.list(&mut env).len(), 0);
}

#[test]
fn public_json_forms_follow_the_protocol() {
    let mut w = world();
    w.now = 1_000;
    let pending = w.pending("a");
    let value = to_value(&pending).unwrap();
    assert_eq!(value["vaultId"], "default");
    assert_eq!(value["state"], "pending");
    assert_eq!(value["expiresUtc"], "1970-01-01T00:05:01.000Z");
    assert_eq!(value["requestId"].as_str().unwrap().len(), 36);
    assert_eq!(value.as_object().unwrap().len(), 4);
    let (queue, mut env) = w.parts();
    let listed = to_value(queue.list(&mut env)).unwrap();
    assert_eq!(listed[0]["deviceName"], "PC factice");
    assert_eq!(listed[0]["challenge"].as_str().unwrap().len(), 44);
    assert_eq!(listed[0]["requestId"], value["requestId"]);
    assert_eq!(listed[0].as_object().unwrap().len(), 6);
}

#[test]
fn denial_consumes_the_request_until_it_expires() {
    let mut w = world();
    let pending = w.pending("a");
    assert_eq!(
        w.decide(&pending.request_id, false, None).unwrap().state,
        RequestState::Denied
    );
    assert_eq!(w.session.unlocks, 0);
    let again = w.pending("a");
    assert_eq!(
        (again.state, again.request_id.as_str()),
        (RequestState::Denied, pending.request_id.as_str())
    );
    let (queue, mut env) = w.parts();
    assert_eq!(queue.list(&mut env).len(), 0);
    w.now = 300_001;
    assert!(w.current("a").is_none());
    assert_eq!(w.pending("a").state, RequestState::Pending);
}

#[test]
fn expiry_lock_revocation_and_account_change_invalidate_the_request() {
    let mut w = world();
    let first = w.pending("a");
    w.now = 300_000;
    assert_eq!(w.decide(&first.request_id, true, None), Err(ErrorCode::RequestExpired));
    let second = w.pending("a");
    w.session.lock();
    assert_eq!(w.decide(&second.request_id, true, None), Err(ErrorCode::RequestExpired));
    w.pending("a");
    w.records.remove(0);
    assert!(w.current("a").is_none());
    let other = w.pending("b");
    w.session.profile = Some("autre-profil".to_owned());
    assert_eq!(w.decide(&other.request_id, true, None), Err(ErrorCode::RequestExpired));
    assert_eq!(w.session.unlocks, 0);
}

#[test]
fn lock_during_approval_keeps_the_vault_closed() {
    let mut w = world();
    let pending = w.pending("a");
    w.session.lock_during_unlock = true;
    assert_eq!(
        w.decide(&pending.request_id, true, None),
        Err(ErrorCode::RequestExpired)
    );
    assert!(!w.session.opened);
    assert!(w.current("a").is_none());
}

#[test]
fn failed_unlock_marks_the_request_failed_and_passes_the_code() {
    let mut w = world();
    let pending = w.pending("a");
    w.session.fail_unlock = true;
    assert_eq!(
        w.decide(&pending.request_id, true, Some("123456")),
        Err(ErrorCode::Locked)
    );
    assert_eq!(w.session.codes, vec![Some("123456".to_owned())]);
    assert_eq!(w.current("a").unwrap().state, RequestState::Failed);
    assert_eq!(w.pending("a").state, RequestState::Failed);
    assert_eq!(
        w.decide(&pending.request_id, true, None),
        Err(ErrorCode::RequestExpired)
    );
}

#[test]
fn unknown_or_malformed_decisions_are_refused() {
    let mut w = world();
    assert_eq!(w.decide("inconnu", true, None), Err(ErrorCode::RequestExpired));
    assert_eq!(w.request("inconnu").err(), Some(ErrorCode::Unauthorized));
    w.session.profile = None;
    assert_eq!(w.request("a").err(), Some(ErrorCode::NotConfigured));
}

#[test]
fn automatic_acceptance_opens_the_vault_at_once() {
    let mut w = world();
    automatic(&mut w, "a");
    let UnlockReply::Request(reply) = w.request("a").unwrap() else {
        panic!("réponse inattendue");
    };
    assert_eq!((reply.state, reply.unlocked), (RequestState::Approved, Some(true)));
    assert_eq!(to_value(&reply).unwrap()["unlocked"], true);
    assert_eq!(w.session.unlocks, 1);
    assert_eq!(w.session.codes, vec![None]);
    w.session.lock();
    assert_eq!(w.pending("b").state, RequestState::Pending);
}

#[test]
fn automatic_acceptance_failure_is_reported_and_recorded() {
    let mut w = world();
    automatic(&mut w, "a");
    w.session.fail_unlock = true;
    assert_eq!(w.request("a").err(), Some(ErrorCode::Locked));
    assert_eq!(w.current("a").unwrap().state, RequestState::Failed);
    w.session.fail_unlock = false;
    assert_eq!(w.pending("a").state, RequestState::Failed);
}

#[test]
fn automatic_acceptance_closed_by_a_lock_during_opening_is_refused() {
    let mut w = world();
    automatic(&mut w, "a");
    w.session.lock_during_unlock = true;
    assert_eq!(w.request("a").err(), Some(ErrorCode::RequestExpired));
    assert!(!w.session.opened);
}

#[test]
fn a_pending_request_is_taken_over_by_automatic_acceptance_without_a_new_id() {
    let mut w = world();
    let pending = w.pending("a");
    automatic(&mut w, "a");
    let UnlockReply::Request(reply) = w.request("a").unwrap() else {
        panic!("réponse inattendue");
    };
    assert_eq!(reply.request_id, pending.request_id);
    assert_eq!(reply.unlocked, Some(true));
}
