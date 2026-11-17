//! Contrôles purs de la cible d'une saisie : identité de la fenêtre active, fenêtre unique, fenêtre activée.

use crate::target_report::{TargetRefusal, WindowDescription};
use crate::target_requirement::TargetRequirement;

/// Instantané d'une fenêtre, indépendant du système : handle, processus, nom du processus et titre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSnapshot {
    /// Identifiant de la fenêtre.
    pub handle: isize,
    /// Identifiant du processus propriétaire.
    pub process_id: u32,
    /// Nom du processus sans `.exe`.
    pub process: String,
    /// Titre de la fenêtre.
    pub title: String,
}

impl WindowSnapshot {
    /// Représentation sérialisable, sans handle.
    ///
    /// Retour : le processus et le titre.
    #[must_use]
    pub fn describe(&self) -> WindowDescription {
        WindowDescription {
            process: self.process.clone(),
            title: self.title.clone(),
        }
    }

    /// Indique si deux instantanés désignent la même fenêtre du même processus.
    ///
    /// Paramètres : l'autre instantané.
    /// Retour : vrai si le handle et le pid sont identiques.
    #[must_use]
    pub fn is_same_window(&self, other: &Self) -> bool {
        self.handle == other.handle && self.process_id == other.process_id
    }

    /// Indique si la fenêtre satisfait les exigences.
    ///
    /// Paramètres : les exigences.
    /// Retour : vrai si le processus et le fragment de titre correspondent.
    #[must_use]
    pub fn satisfies(&self, requirement: &TargetRequirement) -> bool {
        requirement.matches(&self.process, &self.title)
    }
}

/// Construit un refus avec la cible attendue et la fenêtre active.
///
/// Paramètres : la raison, les exigences et la fenêtre active si lisible.
/// Retour : le refus.
fn refusal(reason: &str, requirement: &TargetRequirement, foreground: Option<&WindowSnapshot>) -> TargetRefusal {
    TargetRefusal {
        reason: reason.to_owned(),
        expected: WindowDescription {
            process: requirement.process().to_owned(),
            title: requirement.title().to_owned(),
        },
        foreground: foreground.map(WindowSnapshot::describe),
    }
}

/// Refus rendu quand aucune fenêtre lisible n'est au premier plan.
///
/// Paramètres : les exigences.
/// Retour : le refus, sans fenêtre active.
#[must_use]
pub fn unreadable(requirement: &TargetRequirement) -> TargetRefusal {
    refusal("No readable window is in the foreground.", requirement, None)
}

/// Vérifie que la fenêtre active est exactement celle capturée au lancement et qu'elle satisfait les exigences.
///
/// Paramètres : la fenêtre capturée, la fenêtre active maintenant (`None` si illisible), les exigences.
/// Retour : rien si tout concorde.
///
/// # Errors
///
/// Un refus si la fenêtre active est illisible, a changé, ou ne correspond plus au processus et au titre exigés.
pub fn verify(
    expected: &WindowSnapshot,
    current: Option<&WindowSnapshot>,
    requirement: &TargetRequirement,
) -> Result<(), TargetRefusal> {
    let Some(now) = current else {
        return Err(unreadable(requirement));
    };
    if !now.is_same_window(expected) {
        return Err(refusal(
            "The foreground window changed; click the target field again and retry.",
            requirement,
            current,
        ));
    }
    if !now.satisfies(requirement) {
        return Err(refusal(
            "The foreground window does not match the required process and title; typing refused.",
            requirement,
            current,
        ));
    }
    Ok(())
}

/// Choisit l'unique fenêtre visible qui satisfait les exigences.
///
/// Paramètres : les fenêtres visibles, les exigences.
/// Retour : la fenêtre unique.
///
/// # Errors
///
/// Un refus si aucune fenêtre ne correspond ou si plusieurs correspondent.
pub fn select_unique<'a>(
    windows: &'a [WindowSnapshot],
    requirement: &TargetRequirement,
) -> Result<&'a WindowSnapshot, TargetRefusal> {
    let mut matching = windows.iter().filter(|window| window.satisfies(requirement));
    match (matching.next(), matching.next()) {
        (Some(window), None) => Ok(window),
        (None, _) => Err(refusal(
            "No window matches the required process and title.",
            requirement,
            None,
        )),
        (Some(_), Some(_)) => Err(refusal(
            "Several windows match; make --title more specific.",
            requirement,
            None,
        )),
    }
}

/// Vérifie qu'une fenêtre est bien au premier plan après une demande d'activation.
///
/// Paramètres : la fenêtre visée, le premier plan relu, les exigences.
/// Retour : rien si la fenêtre visée est au premier plan et satisfait toujours les exigences.
///
/// # Errors
///
/// Un refus si Windows n'a pas activé la fenêtre.
pub fn verify_activated(
    target: &WindowSnapshot,
    after: Option<&WindowSnapshot>,
    requirement: &TargetRequirement,
) -> Result<(), TargetRefusal> {
    match after {
        Some(now) if now.is_same_window(target) && now.satisfies(requirement) => Ok(()),
        _ => Err(refusal(
            "Windows refused to bring the window to the foreground.",
            requirement,
            after,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Instantané de test ; le pid se déduit du handle.
    fn window(handle: isize, process: &str, title: &str) -> WindowSnapshot {
        WindowSnapshot {
            handle,
            process_id: u32::try_from(handle).unwrap() + 100,
            process: process.to_owned(),
            title: title.to_owned(),
        }
    }

    /// Exigences habituelles des tests : Chrome sur « Ma banque».
    fn requirement() -> TargetRequirement {
        TargetRequirement::create("chrome", "Ma banque").unwrap()
    }

    #[test]
    fn same_matching_window_passes() {
        let target = window(1, "chrome", "Ma banque - Google Chrome");
        assert!(verify(&target, Some(&target), &requirement()).is_ok());
    }

    #[test]
    fn another_window_is_a_change() {
        let target = window(1, "chrome", "Ma banque");
        let other = window(2, "chrome", "Ma banque");
        let refused = verify(&target, Some(&other), &requirement()).unwrap_err();
        assert!(refused.reason.contains("changed"));
        assert_eq!(refused.foreground, Some(other.describe()));
        assert_eq!(refused.expected.process, "chrome");
        assert_eq!(refused.expected.title, "Ma banque");
    }

    #[test]
    fn same_handle_with_another_process_is_a_change() {
        let target = window(1, "chrome", "Ma banque");
        let mut reused = target.clone();
        reused.process_id += 1;
        assert!(
            verify(&target, Some(&reused), &requirement())
                .unwrap_err()
                .reason
                .contains("changed")
        );
    }

    #[test]
    fn title_drift_is_a_mismatch() {
        let target = window(1, "chrome", "Ma banque");
        let drifted = window(1, "chrome", "Autre site");
        let refused = verify(&target, Some(&drifted), &requirement()).unwrap_err();
        assert!(refused.reason.contains("does not match"));
        assert_eq!(refused.foreground, Some(drifted.describe()));
    }

    #[test]
    fn unreadable_foreground_is_refused_with_a_null_window() {
        let target = window(1, "chrome", "Ma banque");
        let refused = verify(&target, None, &requirement()).unwrap_err();
        assert!(refused.reason.contains("No readable"));
        assert_eq!(refused.foreground, None);
    }

    #[test]
    fn unique_selection_needs_exactly_one_match() {
        let windows = [window(1, "chrome", "Ma banque"), window(2, "notepad", "Ma banque")];
        assert_eq!(select_unique(&windows, &requirement()).unwrap().handle, 1);
        let none = [window(2, "notepad", "Ma banque")];
        assert!(
            select_unique(&none, &requirement())
                .unwrap_err()
                .reason
                .contains("No window")
        );
        assert!(select_unique(&[], &requirement()).is_err());
        let two = [window(1, "chrome", "Ma banque"), window(3, "chrome", "Ma banque 2")];
        assert!(
            select_unique(&two, &requirement())
                .unwrap_err()
                .reason
                .contains("Several")
        );
    }

    #[test]
    fn activation_is_checked_after_the_request() {
        let target = window(1, "chrome", "Ma banque");
        assert!(verify_activated(&target, Some(&target), &requirement()).is_ok());
        let other = window(2, "notepad", "x");
        let refused = verify_activated(&target, Some(&other), &requirement()).unwrap_err();
        assert!(refused.reason.contains("refused"));
        assert_eq!(refused.foreground, Some(other.describe()));
        assert!(verify_activated(&target, None, &requirement()).is_err());
    }
}
