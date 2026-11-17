//! Tests Win32 réels, ignorés par défaut (`cargo test -- --ignored`) : ils ne tapent que dans leur fenêtre factice.
//!
//! Règle : le contrôle de cible de chaque frappe exige que la fenêtre au premier plan soit la fenêtre factice du
//! test ; sinon aucun événement n'est envoyé, et le test le rapporte sans taper.

use std::time::Duration;

use zeroize::Zeroizing;

use crate::input::{TypeError, type_text};
use crate::raise::bring_to_front;
use crate::test_window::{TestWindow, pump};
use crate::window::{describe, foreground, visible_windows};

/// Valeur factice tapée par les tests.
const DUMMY: &str = "valeur-factice-42-\u{e9}";

/// Titre unique de la fenêtre factice de ce processus.
fn unique_title(tag: &str) -> String {
    format!("VaultAgent fenetre factice {tag} {}", std::process::id())
}

/// Indique si la fenêtre factice est au premier plan.
fn is_foreground(window: &TestWindow) -> bool {
    foreground().is_ok_and(|current| current.handle == window.handle)
}

/// Met la fenêtre factice au premier plan avec le focus sur son champ.
fn take_foreground(window: &TestWindow) -> bool {
    bring_to_front(window.handle).unwrap_or_default();
    window.focus_edit();
    pump(Duration::from_millis(200));
    is_foreground(window)
}

/// Contrôle de cible des tests : la fenêtre au premier plan doit être la fenêtre factice.
fn only_this_window(window: &TestWindow) -> Result<(), &'static str> {
    if is_foreground(window) {
        Ok(())
    } else {
        Err("the foreground window is not the test window")
    }
}

#[test]
#[ignore = "fenetre factice du test"]
fn typing_reaches_the_fake_window_and_reads_back() {
    let Some(window) = TestWindow::open(&unique_title("saisie")) else {
        println!("RAPPORT : fenetre factice non creee, aucune frappe");
        return;
    };
    if !take_foreground(&window) {
        println!("RAPPORT : premier plan refuse par Windows, aucune frappe");
        return;
    }
    let text = Zeroizing::new(DUMMY.to_owned());
    let typed = type_text(&text, || only_this_window(&window));
    pump(Duration::from_millis(600));
    assert_eq!(typed, Ok(DUMMY.chars().count()));
    assert_eq!(window.text(), DUMMY);
}

#[test]
#[ignore = "fenetre factice du test"]
fn typing_is_refused_when_another_window_is_in_front() {
    let Some(target) = TestWindow::open(&unique_title("cible")) else {
        println!("RAPPORT : fenetre cible non creee");
        return;
    };
    let Some(other) = TestWindow::open(&unique_title("autre")) else {
        println!("RAPPORT : seconde fenetre non creee");
        return;
    };
    if !take_foreground(&other) {
        println!("RAPPORT : premier plan non pris par la seconde fenetre, test non concluant");
        return;
    }
    let text = Zeroizing::new(DUMMY.to_owned());
    let result = type_text(&text, || only_this_window(&target));
    pump(Duration::from_millis(600));
    assert_eq!(
        result,
        Err(TypeError::Refused {
            reason: "the foreground window is not the test window",
            typed: 0
        })
    );
    assert_eq!(target.text(), "");
    assert_eq!(other.text(), "");
}

#[test]
#[ignore = "fenetre factice du test"]
fn typing_stops_as_soon_as_the_check_refuses() {
    let Some(window) = TestWindow::open(&unique_title("arret")) else {
        println!("RAPPORT : fenetre factice non creee, aucune frappe");
        return;
    };
    if !take_foreground(&window) {
        println!("RAPPORT : premier plan refuse par Windows, aucune frappe");
        return;
    }
    let text = Zeroizing::new("abcdef".to_owned());
    let mut calls = 0;
    let result = type_text(&text, || {
        calls += 1;
        if calls > 3 {
            Err("window changed")
        } else {
            only_this_window(&window)
        }
    });
    pump(Duration::from_millis(600));
    assert_eq!(
        result,
        Err(TypeError::Refused {
            reason: "window changed",
            typed: 3
        })
    );
    assert_eq!(window.text(), "abc");
}

#[test]
#[ignore = "fenetre factice du test"]
fn the_fake_window_is_listed_and_described() {
    let title = unique_title("liste");
    let Some(window) = TestWindow::open(&title) else {
        println!("RAPPORT : fenetre factice non creee");
        return;
    };
    pump(Duration::from_millis(200));
    let listed = visible_windows()
        .into_iter()
        .find(|entry| entry.handle == window.handle);
    let described = describe(window.handle);
    assert_eq!(listed, described);
    assert!(
        described
            .as_ref()
            .is_some_and(|info| info.title == title && info.process_id == std::process::id())
    );
}
