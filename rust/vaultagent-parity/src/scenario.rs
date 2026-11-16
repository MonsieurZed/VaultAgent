//! Le scénario complet, rejoué à l'identique sur chaque serveur.

use crate::run::{Run, object};
use crate::scenario_pair::Devices;
use crate::{scenario_admin, scenario_agent, scenario_errors, scenario_lock, scenario_pair};
use serde_json::Value;

/// Révoque les deux appareils, vérifie que le jeton révoqué est refusé, puis verrouille le coffre.
fn finish(run: &mut Run<'_>, devices: &Devices) {
    run.admin_post(
        "admin-revoke-second",
        "revoke",
        &object([("deviceId", Value::from(devices.second.as_str()))]),
    );
    run.admin_post(
        "admin-revoke-first",
        "revoke",
        &object([("deviceId", Value::from(devices.first.as_str()))]),
    );
    run.agent_post("agent-status-revoked", "status", &object([]));
    run.admin_get("admin-state-final", "/api/admin/state");
    run.admin_post("admin-lock", "lock", &object([]));
}

/// Joue le scénario dans l'ordre : admin, association, routes agent, verrouillage, erreurs, fin.
///
/// Paramètres : le passage, dont les échanges se remplissent.
/// Retour : rien.
pub fn play(run: &mut Run<'_>) {
    let code = scenario_admin::setup(run);
    let devices = scenario_pair::flow(run, &code);
    scenario_agent::routes(run);
    scenario_lock::cycle(run);
    scenario_errors::cases(run);
    finish(run, &devices);
}
