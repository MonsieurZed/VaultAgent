//! `GET /` : page complète rendue côté serveur depuis l'état administrateur du coffre `default`.

use crate::admin_state::snapshot;
use crate::context::Context;
use crate::error::Result;
use crate::front_html::View;
use crate::front_lang::Lang;
use crate::front_notice::Notice;
use crate::front_response::page;
use crate::front_screen::Screen;
use crate::front_view::render;
use crate::http::{Request, Response};
use crate::query::query_param;

/// Rend la page principale, le mode « Modifier » `?view=vault` (d'office tant que rien n'est
/// configuré) ou l'aide `?view=help`, avec `?msg=`, `?lang=`, `?page=` et `?confirm=` ; un `?lang=` valide est mémorisé dans le cookie. Un ancien `?tab=` est ignoré.
///
/// Paramètres : le contexte, la requête (propriétaire déjà contrôlé).
/// Retour : la réponse HTML.
///
/// # Errors
///
/// `vault-missing` ou une erreur de lecture de l'état.
pub fn serve(context: &Context, request: &Request) -> Result<Response> {
    let requested = query_param(&request.path, "lang").and_then(|code| Lang::parse(&code));
    let lang = Lang::choose(requested.map(Lang::code), request.header("cookie"));
    let requested_screen = query_param(&request.path, "view")
        .and_then(|code| Screen::parse(&code))
        .unwrap_or(Screen::Main);
    let notice = query_param(&request.path, "msg").and_then(|code| Notice::parse(&code));
    let vault = context.vaults.get(None)?;
    let state = snapshot(context, &vault)?;
    let screen = if requested_screen == Screen::Main && !state.session.configured {
        Screen::Vault
    } else {
        requested_screen
    };
    let confirm =
        query_param(&request.path, "confirm").filter(|id| state.devices.iter().any(|device| &device.id == id));
    let view = View {
        lang,
        screen,
        notice,
        page: query_param(&request.path, "page"),
        confirm,
        state: &state,
    };
    Ok(page(render(&view), requested))
}
