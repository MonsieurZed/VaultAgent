//! Association de deux appareils : par demande approuvée par l'administrateur (appareil A, approbateur) et par code
//! (appareil B).

use crate::run::{Run, field, object};
use serde_json::Value;

/// Identifiants des appareils associés, pour la révocation finale.
pub struct Devices {
    /// Appareil A (demande, approbateur).
    pub first: String,
    /// Appareil B (code).
    pub second: String,
}

/// Joue les deux associations et règle le jeton de l'appareil A dans le passage.
///
/// Paramètres : le passage et le code d'association émis par l'administrateur.
/// Retour : les identifiants des deux appareils.
pub fn flow(run: &mut Run<'_>, code: &str) -> Devices {
    let request = object([
        ("action", Value::from("request")),
        ("name", Value::from("parity-client")),
        ("approverPublicKey", Value::from(run.settings.public_key.as_str())),
    ]);
    let opened = run.pair_post("pair-request", &request);
    let request_id = field(&opened, "requestId");
    let secret = field(&opened, "secret");
    let poll = object([
        ("action", Value::from("poll")),
        ("requestId", Value::from(request_id.as_str())),
        ("secret", Value::from(secret.as_str())),
    ]);
    run.pair_post("pair-poll-pending", &poll);
    run.admin_get("admin-state-pair-pending", "/api/admin/state");
    let decision = object([
        ("requestId", Value::from(request_id.as_str())),
        ("approve", Value::Bool(true)),
    ]);
    run.admin_post("admin-pair-decide", "pair-decide", &decision);
    let approved = run.pair_post("pair-poll-approved", &poll);
    run.pair_post("pair-poll-consumed", &poll);
    run.bearer = format!("Bearer {}", field(&approved, "token"));
    run.vault_id = field(&approved, "vaultId");
    let by_code = run.pair_post("pair-code", &object([("code", Value::from(code))]));
    run.pair_post("pair-code-reused", &object([("code", Value::from(code))]));
    run.pair_post(
        "pair-code-refused",
        &object([("code", Value::from("default.invalid-code"))]),
    );
    Devices {
        first: field(&approved, "deviceId"),
        second: field(&by_code, "deviceId"),
    }
}
