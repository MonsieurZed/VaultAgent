//! Fausse fenêtre des tests de saisie : premier plan scripté, fenêtres visibles, frappe et activations notées.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::cell::{Cell, RefCell};

use serde_json::Value;
use vaultagent_client::error::ClientError;
use vaultagent_client::target_check::WindowSnapshot;
use vaultagent_client::window_args::WindowArgs;
use vaultagent_client::window_system::{TargetCheck, WindowSystem};
use zeroize::Zeroizing;

/// Fausse fenêtre : premier plan scripté, fenêtres visibles, frappe notée.
pub struct FakeWindows {
    script: Vec<Option<WindowSnapshot>>,
    /// Nombre de lectures du premier plan.
    pub reads: Cell<usize>,
    visible: Vec<WindowSnapshot>,
    /// Caractères « tapés » après le contrôle de cible.
    pub typed: RefCell<String>,
    /// Fenêtres dont l'activation a été demandée.
    pub raised: RefCell<Vec<isize>>,
}

impl FakeWindows {
    /// Crée une fausse fenêtre ; chaque lecture du premier plan rend l'entrée suivante du script, la dernière ensuite.
    pub fn new(script: Vec<Option<WindowSnapshot>>, visible: Vec<WindowSnapshot>) -> Self {
        Self {
            script,
            reads: Cell::new(0),
            visible,
            typed: RefCell::new(String::new()),
            raised: RefCell::new(Vec::new()),
        }
    }
}

impl WindowSystem for FakeWindows {
    /// Rend l'entrée suivante du script, la dernière ensuite.
    fn foreground(&self) -> Option<WindowSnapshot> {
        let index = self.reads.get().min(self.script.len() - 1);
        self.reads.set(self.reads.get() + 1);
        self.script[index].clone()
    }

    /// Rend les fenêtres visibles prévues.
    fn visible(&self) -> Vec<WindowSnapshot> {
        self.visible.clone()
    }

    /// Note la demande d'activation.
    fn bring_to_front(&self, window: &WindowSnapshot) {
        self.raised.borrow_mut().push(window.handle);
    }

    /// Note chaque caractère « tapé » après le contrôle de cible.
    fn type_text(&self, text: &Zeroizing<String>, check: TargetCheck<'_>) -> Result<(), ClientError> {
        for character in text.chars() {
            check().map_err(|refusal| ClientError::TargetRefused(Box::new(refusal)))?;
            self.typed.borrow_mut().push(character);
        }
        Ok(())
    }
}

/// Instantané de fenêtre de test ; le pid se déduit du handle.
pub fn window(handle: isize, process: &str, title: &str) -> WindowSnapshot {
    WindowSnapshot {
        handle,
        process_id: 500 + u32::try_from(handle).unwrap(),
        process: process.to_owned(),
        title: title.to_owned(),
    }
}

/// Fenêtre cible habituelle des tests : Chrome sur « Ma banque ».
pub fn bank() -> WindowSnapshot {
    window(1, "chrome", "Ma banque - Google Chrome")
}

/// Arguments de cible `--process` et `--title`.
pub fn target(process: &str, title: &str) -> WindowArgs {
    WindowArgs {
        process: process.to_owned(),
        title: title.to_owned(),
    }
}

/// Lit une ligne de sortie comme JSON.
pub fn json(line: &str) -> Value {
    serde_json::from_str(line).unwrap()
}
