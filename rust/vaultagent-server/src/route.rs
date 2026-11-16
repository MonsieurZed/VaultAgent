//! Cœur HTTP pur du serveur : `route(&Request, &Context) -> Response`, sans aucune dépendance au transport.
//!
//! Ordre des contrôles (`docs/rust/protocole.md`) : `GET /healthz`, proxy de confiance, origine, puis le chemin.
//! Les routes agent sont traitées par [`crate::agent`], `POST /api/pair` par [`crate::pair_route`] ; tout le reste
//! (fichiers, `/api/admin/*`) exige le propriétaire Authentik puis relève de [`crate::owner`].

use serde::Serialize;
use vaultagent_core::ErrorCode;

use crate::agent::handle;
use crate::context::Context;
use crate::error::Result;
use crate::http::{Request, Response};
use crate::owner;
use crate::pair_route;

const PAIR_PATH: &str = "/api/pair";
const AGENT_PREFIX: &str = "/api/agent/";
const MAPPED_IPV4: &str = "::ffff:";

/// Réponse de `GET /healthz`.
#[derive(Serialize)]
struct Alive {
    alive: bool,
}

/// Répond à une requête HTTP.
///
/// Paramètres : la requête reçue, le contexte du service.
/// Retour : la réponse, toujours en JSON, y compris pour les erreurs.
#[must_use]
pub fn route(request: &Request, context: &Context) -> Response {
    dispatch(request, context).unwrap_or_else(|error| Response::error(&error))
}

/// Applique les contrôles communs puis aiguille vers la route.
fn dispatch(request: &Request, context: &Context) -> Result<Response> {
    let path = request.route_path();
    if request.method == "GET" && path == "/healthz" {
        return Ok(Response::json(200, &Alive { alive: true }));
    }
    let remote = request.remote.strip_prefix(MAPPED_IPV4).unwrap_or(&request.remote);
    if remote != context.trusted_proxy {
        return Err(ErrorCode::ProxyRequired.into());
    }
    if request
        .header("origin")
        .is_some_and(|origin| !origin.is_empty() && origin != context.origin)
    {
        return Err(ErrorCode::OriginRefused.into());
    }
    if request.method == "POST" && path == PAIR_PATH {
        return pair_route::handle(context, request);
    }
    if let Some(name) = path.strip_prefix(AGENT_PREFIX) {
        return handle(context, request, name);
    }
    let owner = request.header("x-authentik-username");
    if context.owner.is_none() || owner != context.owner.as_deref() {
        return Err(ErrorCode::OwnerRequired.into());
    }
    owner::handle(context, request, path)
}
