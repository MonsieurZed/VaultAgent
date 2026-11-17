//! Commandes `foreground` et `focus` contre une fausse fenêtre ; lecture seule réelle sous Windows.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::rc::Rc;

use common::fake_windows::{FakeWindows, bank, json, target, window};
use vaultagent_client::error::ClientError;
use vaultagent_client::exit_code::ExitCode;
use vaultagent_client::fake_transport::FakeTransport;
use vaultagent_client::window_commands::{focus, foreground};

#[test]
fn foreground_describes_the_active_window() {
    let system = FakeWindows::new(vec![Some(bank())], vec![]);
    let outcome = foreground(&system).unwrap();
    assert_eq!(outcome.exit, ExitCode::Success);
    assert_eq!(
        json(&outcome.line),
        json("{\"process\":\"chrome\",\"title\":\"Ma banque - Google Chrome\"}")
    );
    let empty = FakeWindows::new(vec![None], vec![]);
    assert!(matches!(foreground(&empty), Err(ClientError::Usage(_))));
}

#[test]
fn focus_activates_the_single_matching_window_and_checks_the_result() {
    let visible = vec![window(7, "notepad", "Ma banque"), bank()];
    let system = FakeWindows::new(vec![Some(bank())], visible);
    let outcome = focus(&system, &target("chrome", "Ma banque")).unwrap();
    assert_eq!(outcome.exit, ExitCode::Success);
    assert_eq!(*system.raised.borrow(), vec![1]);
    let report = json(&outcome.line);
    assert_eq!(report["focused"], true);
    assert_eq!(report["target"]["process"], "chrome");
}

#[test]
fn focus_reports_a_refusal_when_windows_does_not_activate() {
    let other = window(9, "notepad", "Sans titre");
    let system = FakeWindows::new(vec![Some(other)], vec![bank()]);
    let outcome = focus(&system, &target("chrome", "Ma banque")).unwrap();
    assert_eq!(outcome.exit, ExitCode::TargetRefused);
    let report = json(&outcome.line);
    assert_eq!(report["focused"], false);
    assert!(report["reason"].as_str().unwrap().contains("refused"));
    assert_eq!(report["foreground"]["process"], "notepad");
}

#[test]
fn focus_refuses_zero_or_several_matches_without_raising_anything() {
    let none = FakeWindows::new(vec![Some(bank())], vec![window(7, "notepad", "Ma banque")]);
    let outcome = focus(&none, &target("chrome", "Ma banque")).unwrap();
    assert_eq!(outcome.exit, ExitCode::TargetRefused);
    assert!(json(&outcome.line)["reason"].as_str().unwrap().contains("No window"));
    let two = FakeWindows::new(vec![Some(bank())], vec![bank(), window(2, "chrome", "Ma banque 2")]);
    let outcome = focus(&two, &target("chrome", "Ma banque")).unwrap();
    assert_eq!(outcome.exit, ExitCode::TargetRefused);
    assert!(json(&outcome.line)["reason"].as_str().unwrap().contains("Several"));
    assert!(none.raised.borrow().is_empty() && two.raised.borrow().is_empty());
}

#[test]
fn focus_rejects_a_generic_title() {
    let system = FakeWindows::new(vec![Some(bank())], vec![bank()]);
    assert!(matches!(
        focus(&system, &target("chrome", "New Tab")),
        Err(ClientError::Usage(_))
    ));
}

#[cfg(windows)]
#[test]
fn the_real_foreground_command_answers_or_fails_cleanly() {
    let outcome = common::execute(&Rc::new(FakeTransport::new()), &["foreground"]);
    if outcome.exit == ExitCode::Success {
        let report = json(outcome.out.trim());
        assert!(report["process"].is_string() && report["title"].is_string());
    } else {
        assert_eq!(outcome.exit, ExitCode::Usage);
        assert_eq!(json(outcome.err.trim())["version"], 1);
    }
}

#[cfg(windows)]
#[test]
fn the_real_focus_command_refuses_an_unknown_window_without_touching_anything() {
    let args = [
        "focus",
        "--process",
        "no-such-process-4242",
        "--title",
        "no-such-window-4242",
    ];
    let outcome = common::execute(&Rc::new(FakeTransport::new()), &args);
    assert_eq!(outcome.exit, ExitCode::TargetRefused);
    let report = json(outcome.out.trim());
    assert_eq!(report["focused"], false);
    assert_eq!(report["expected"]["process"], "no-such-process-4242");
}

#[cfg(not(windows))]
#[test]
fn window_commands_are_refused_outside_windows() {
    let outcome = common::execute(&Rc::new(FakeTransport::new()), &["foreground"]);
    assert_eq!(outcome.exit, ExitCode::Usage);
    assert_eq!(json(outcome.err.trim())["error"], "usage");
}
