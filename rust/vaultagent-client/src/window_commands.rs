//! Commandes `foreground`, `focus` et `type` : orchestration pure au-dessus de `WindowSystem`.

use crate::config::ClientConfig;
use crate::error::ClientError;
use crate::exit_code::ExitCode;
use crate::fill::fill_with;
use crate::seal::check_target;
use crate::target_check::{select_unique, unreadable, verify, verify_activated};
use crate::target_report::{
    TargetRefusal, focus_refusal_line, focused_line, foreground_line, typed_line, typed_refusal_line,
};
use crate::target_requirement::TargetRequirement;
use crate::transport::Transport;
use crate::unlock::Outcome;
use crate::window_args::{TypeArgs, WindowArgs};
use crate::window_system::WindowSystem;

/// Enveloppe une ligne de sortie avec un code de sortie.
fn outcome(line: String, exit: ExitCode) -> Outcome {
    Outcome { line, exit }
}

/// Construit les exigences depuis les arguments `--process` et `--title`.
///
/// Paramètres : les arguments de fenêtre.
/// Retour : les exigences validées.
///
/// # Errors
///
/// `Usage` si le processus ou le titre est invalide ou trop générique.
fn requirement(args: &WindowArgs) -> Result<TargetRequirement, ClientError> {
    TargetRequirement::create(&args.process, &args.title)
}

/// Exécute `foreground` : décrit la fenêtre active pour choisir `--process` et `--title`.
///
/// Paramètres : le système de fenêtres.
/// Retour : `{"process":…,"title":…}`, code 0.
///
/// # Errors
///
/// `Usage` si aucune fenêtre lisible n'est au premier plan.
pub fn foreground(system: &dyn WindowSystem) -> Result<Outcome, ClientError> {
    let window = system
        .foreground()
        .ok_or_else(|| ClientError::Usage("No readable window is in the foreground.".to_owned()))?;
    Ok(outcome(foreground_line(&window.describe())?, ExitCode::Success))
}

/// Exécute `focus` : met au premier plan l'unique fenêtre correspondant à la cible, puis relit le premier plan.
///
/// Paramètres : le système de fenêtres et la cible.
/// Retour : `{"focused":true,"target":…}` code 0, ou le rapport de refus code 3.
///
/// # Errors
///
/// `Usage` pour une cible invalide.
pub fn focus(system: &dyn WindowSystem, args: &WindowArgs) -> Result<Outcome, ClientError> {
    let requirement = requirement(args)?;
    let windows = system.visible();
    let target = match select_unique(&windows, &requirement) {
        Ok(window) => window,
        Err(refusal) => return refused_focus(&refusal),
    };
    system.bring_to_front(target);
    let after = system.foreground();
    match verify_activated(target, after.as_ref(), &requirement) {
        Ok(()) => Ok(outcome(focused_line(&target.describe())?, ExitCode::Success)),
        Err(refusal) => refused_focus(&refusal),
    }
}

/// Rapport de refus d'une mise au premier plan.
///
/// Paramètres : le refus.
/// Retour : le rapport sur stdout, code 3.
fn refused_focus(refusal: &TargetRefusal) -> Result<Outcome, ClientError> {
    Ok(outcome(focus_refusal_line(refusal)?, ExitCode::TargetRefused))
}

/// Rapport de refus d'une frappe.
///
/// Paramètres : le refus.
/// Retour : le rapport sur stdout, code 3.
fn refused_typing(refusal: &TargetRefusal) -> Result<Outcome, ClientError> {
    Ok(outcome(typed_refusal_line(refusal)?, ExitCode::TargetRefused))
}

/// Exécute `type` : vérifie la cible, demande l'enveloppe, la tape caractère par caractère sous contrôle, l'efface.
///
/// Paramètres : le système de fenêtres, le transport, la configuration, le coffre choisi, les arguments.
/// Retour : `{"typed":true,"field":…,"clipboardUsed":false,"target":…}` code 0, ou le rapport de refus code 3
/// (cible non conforme avant la demande, ou premier plan changé pendant la frappe).
///
/// # Errors
///
/// `Usage` pour un argument invalide ; l'erreur classée du serveur ; `Window` si Windows refuse la frappe.
pub fn type_text(
    system: &dyn WindowSystem,
    transport: &dyn Transport,
    config: &ClientConfig,
    vault: Option<&str>,
    args: &TypeArgs,
) -> Result<Outcome, ClientError> {
    check_target(vault, &args.item_id, &args.field, &args.origin)?;
    let requirement = requirement(&args.window)?;
    let Some(expected) = system.foreground() else {
        return refused_typing(&unreadable(&requirement));
    };
    if let Err(refusal) = verify(&expected, Some(&expected), &requirement) {
        return refused_typing(&refusal);
    }
    let result = fill_with(
        transport,
        config,
        vault,
        &args.item_id,
        &args.field,
        &args.origin,
        |text| {
            system.type_text(text, &mut || {
                verify(&expected, system.foreground().as_ref(), &requirement)
            })
        },
    );
    match result {
        Ok(report) => Ok(outcome(
            typed_line(&report.field, &expected.describe())?,
            ExitCode::Success,
        )),
        Err(ClientError::TargetRefused(refusal)) => refused_typing(&refusal),
        Err(other) => Err(other),
    }
}
