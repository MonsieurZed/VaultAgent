//! Cas d'erreur du protocole : origine refusée, JSON requis, route refusée, méthode refusée, non autorisé.

use crate::run::{Run, object};

const FAKE_TOKEN: &str = "Bearer va1.00000000-0000-4000-8000-000000000000.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const EVIL_ORIGIN: &str = "https://evil.invalid";

/// Construit une liste d'en-têtes à partir de paires de textes.
fn headers(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

/// Joue les requêtes refusées, dans l'ordre des contrôles du protocole.
///
/// Paramètres : le passage, associé.
/// Retour : rien.
pub fn cases(run: &mut Run<'_>) {
    let bearer = run.bearer.clone();
    let owner = run.settings.owner.clone();
    let json = "application/json";
    run.raw(
        "error-agent-origin-refused",
        "POST",
        "/api/agent/status",
        headers(&[
            ("authorization", &bearer),
            ("content-type", json),
            ("origin", EVIL_ORIGIN),
        ]),
        Some("{}".to_owned()),
    );
    run.raw(
        "error-agent-unauthorized",
        "POST",
        "/api/agent/status",
        headers(&[("authorization", FAKE_TOKEN), ("content-type", json)]),
        Some("{}".to_owned()),
    );
    run.raw(
        "error-agent-no-token",
        "POST",
        "/api/agent/status",
        headers(&[("content-type", json)]),
        Some("{}".to_owned()),
    );
    run.raw(
        "error-agent-method-refused",
        "GET",
        "/api/agent/status",
        headers(&[("authorization", &bearer)]),
        None,
    );
    run.raw(
        "error-agent-json-required",
        "POST",
        "/api/agent/status",
        headers(&[("authorization", &bearer), ("content-type", "text/plain")]),
        Some("{}".to_owned()),
    );
    run.raw(
        "error-agent-invalid-json",
        "POST",
        "/api/agent/status",
        headers(&[("authorization", &bearer), ("content-type", json)]),
        Some("{".to_owned()),
    );
    run.agent_post("error-agent-route-refused", "inconnue", &object([]));
    run.raw(
        "error-admin-origin-refused",
        "POST",
        "/api/admin/lock",
        headers(&[
            ("x-authentik-username", &owner),
            ("content-type", json),
            ("origin", EVIL_ORIGIN),
        ]),
        Some("{}".to_owned()),
    );
    run.raw(
        "error-admin-owner-required",
        "GET",
        "/api/admin/state",
        Vec::new(),
        None,
    );
    run.raw(
        "error-admin-wrong-owner",
        "POST",
        "/api/admin/lock",
        headers(&[("x-authentik-username", "intrus"), ("content-type", json)]),
        Some("{}".to_owned()),
    );
    run.raw(
        "error-admin-get-origin-refused",
        "GET",
        "/api/admin/state",
        headers(&[("x-authentik-username", &owner), ("origin", EVIL_ORIGIN)]),
        None,
    );
    run.raw(
        "error-download-unavailable",
        "GET",
        "/downloads/windows-client.zip",
        headers(&[("x-authentik-username", &owner)]),
        None,
    );
    run.pair_post("error-pair-invalid-action", &object([("action", "inconnue".into())]));
}
