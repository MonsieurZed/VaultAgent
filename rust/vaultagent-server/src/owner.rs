//! Routes réservées au propriétaire (en-tête Authentik déjà contrôlé) : fichiers, page, état, formulaires et actions
//! `POST`.

use serde::Serialize;
use vaultagent_core::ErrorCode;

use crate::admin;
use crate::admin_state;
use crate::context::Context;
use crate::error::Result;
use crate::files;
use crate::front_action;
use crate::front_page;
use crate::http::{Request, Response};

/// Corps de la réponse `404` des anciens fichiers du front.
#[derive(Serialize)]
struct NotFound {
    error: &'static str,
}

/// Aiguille une requête du propriétaire, dans l'ordre de Node : fichiers, état, contrôle d'origine, action.
///
/// Paramètres : le contexte, la requête, le chemin sans requête.
/// Retour : la réponse.
///
/// # Errors
///
/// `origin-refused` pour toute requête qui n'est pas un `POST` portant l'origine du service, puis les erreurs de la
/// route d'administration.
pub fn handle(context: &Context, request: &Request, path: &str) -> Result<Response> {
    if request.method == "GET" {
        if let Some(served) = files::serve(context, path) {
            return Ok(served);
        }
        match path {
            "/" => return front_page::serve(context, request),
            "/api/admin/state" => return admin_state::state(context, request),
            "/app.js" | "/index.html" => return Ok(Response::json(404, &NotFound { error: "not-found" })),
            _ => {}
        }
    }
    if request.method == "POST" && path.starts_with("/admin/") {
        return front_action::handle(context, request, path);
    }
    if request.method != "POST" || request.header("origin") != Some(context.origin.as_str()) {
        return Err(ErrorCode::OriginRefused.into());
    }
    admin::post(context, request, path)
}
