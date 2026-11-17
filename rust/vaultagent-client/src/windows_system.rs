//! Système de fenêtres Windows : adapte `vaultagent-win32` au trait `WindowSystem`.

use vaultagent_win32::{TypeError, WindowInfo};
use zeroize::Zeroizing;

use crate::error::ClientError;
use crate::target_check::WindowSnapshot;
use crate::window_system::{TargetCheck, WindowSystem};

/// Fenêtres du poste Windows courant.
pub struct WindowsSystem;

/// Convertit l'identité Win32 en instantané indépendant du système.
///
/// Paramètres : l'identité lue par `vaultagent-win32`.
/// Retour : l'instantané.
fn snapshot(info: WindowInfo) -> WindowSnapshot {
    WindowSnapshot {
        handle: info.handle,
        process_id: info.process_id,
        process: info.process,
        title: info.title,
    }
}

impl WindowSystem for WindowsSystem {
    /// Lit le premier plan ; `None` si aucune fenêtre n'est active ou si son processus est illisible.
    fn foreground(&self) -> Option<WindowSnapshot> {
        vaultagent_win32::foreground().ok().map(snapshot)
    }

    /// Liste les fenêtres visibles dont le processus est lisible.
    fn visible(&self) -> Vec<WindowSnapshot> {
        vaultagent_win32::visible_windows().into_iter().map(snapshot).collect()
    }

    /// Demande l'activation ; un refus de Windows apparaît à la relecture du premier plan.
    fn bring_to_front(&self, window: &WindowSnapshot) {
        vaultagent_win32::bring_to_front(window.handle).unwrap_or_default();
    }

    /// Tape par `SendInput` en appelant `check` avant chaque caractère.
    fn type_text(&self, text: &Zeroizing<String>, check: TargetCheck<'_>) -> Result<(), ClientError> {
        vaultagent_win32::type_text(text, check)
            .map(|_| ())
            .map_err(|error| match error {
                TypeError::Refused { reason, .. } => ClientError::TargetRefused(Box::new(reason)),
                TypeError::Win32(failure) => ClientError::Window(failure.to_string()),
            })
    }
}
