//! Étapes administrateur du début de scénario : santé, état, configuration du compte factice, déverrouillage et
//! émission d'un code d'association.

use crate::run::{Run, field, object};
use serde_json::Value;

const SESSION_MINUTES: u32 = 15;

/// Joue la santé puis la configuration du coffre.
///
/// Paramètres : le passage.
/// Retour : le code d'association émis par l'administrateur (`<coffre>.<code>`), vide si l'étape a échoué.
pub fn setup(run: &mut Run<'_>) -> String {
    run.raw("healthz", "GET", "/healthz", Vec::new(), None);
    run.admin_get("admin-state-initial", "/api/admin/state");
    let configure = object([
        ("server", Value::from(run.settings.server.as_str())),
        ("email", Value::from(run.settings.email.as_str())),
        ("password", Value::from(run.settings.password.as_str())),
        ("timeoutMinutes", Value::from(SESSION_MINUTES)),
    ]);
    run.admin_post("admin-configure", "configure", &configure);
    run.admin_get("admin-state-configured", "/api/admin/state");
    run.admin_post("admin-unlock", "unlock", &object([]));
    run.admin_get("admin-state-unlocked", "/api/admin/state");
    let reply = run.admin_post(
        "admin-pairing",
        "pairing",
        &object([("name", Value::from("parity-code"))]),
    );
    field(&reply, "code")
}
