//! Accès aux fenêtres du système, derrière un trait que les tests remplacent par une fausse fenêtre.

use zeroize::Zeroizing;

use crate::error::ClientError;
use crate::target_check::WindowSnapshot;
use crate::target_report::TargetRefusal;

/// Contrôle de cible appelé avant chaque caractère tapé.
pub type TargetCheck<'a> = &'a mut dyn FnMut() -> Result<(), TargetRefusal>;

/// Opérations de fenêtre dont les commandes `foreground`, `focus` et `type` ont besoin.
pub trait WindowSystem {
    /// Lit la fenêtre au premier plan.
    ///
    /// Retour : son instantané, ou `None` si aucune fenêtre lisible n'est active.
    fn foreground(&self) -> Option<WindowSnapshot>;

    /// Liste les fenêtres visibles de premier niveau.
    ///
    /// Retour : leurs instantanés.
    fn visible(&self) -> Vec<WindowSnapshot>;

    /// Demande la mise au premier plan d'une fenêtre ; le résultat se lit ensuite par `foreground`.
    ///
    /// Paramètres : la fenêtre visée.
    fn bring_to_front(&self, window: &WindowSnapshot);

    /// Tape un texte caractère par caractère ; `check` est appelé avant chacun.
    ///
    /// Paramètres : le texte secret et le contrôle de cible.
    ///
    /// # Errors
    ///
    /// `TargetRefused` dès que le contrôle refuse (plus rien n'est tapé), `Window` si le système refuse la frappe.
    fn type_text(&self, text: &Zeroizing<String>, check: TargetCheck<'_>) -> Result<(), ClientError>;
}

/// Système de fenêtres réel de la plateforme.
///
/// Retour : le système Windows.
///
/// # Errors
///
/// `Usage` hors de Windows : ces commandes n'y existent pas.
#[cfg(windows)]
pub fn system() -> Result<Box<dyn WindowSystem>, ClientError> {
    Ok(Box::new(crate::windows_system::WindowsSystem))
}

/// Système de fenêtres réel de la plateforme.
///
/// Retour : jamais de système.
///
/// # Errors
///
/// `Usage` hors de Windows : ces commandes n'y existent pas.
#[cfg(not(windows))]
pub fn system() -> Result<Box<dyn WindowSystem>, ClientError> {
    Err(ClientError::Usage(
        "Window commands (foreground, focus, type) are available on Windows only.".to_owned(),
    ))
}
