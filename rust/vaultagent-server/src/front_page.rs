//! `GET /` : page complète rendue côté serveur depuis l'état administrateur du coffre `default`.

use crate::admin_state::snapshot;
use crate::context::Context;
use crate::error::Result;
use crate::front_html::View;
use crate::front_lang::Lang;
use crate::front_notice::Notice;
use crate::front_response::page;
use crate::front_tab::Tab;
use crate::front_view::render;
use crate::http::{Request, Response};
use crate::query::query_param;

/// Rend la page selon `?tab=`, `?msg=` et `?lang=` ; un `?lang=` valide est mémorisé dans le cookie.
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
    let tab = query_param(&request.path, "tab")
        .and_then(|code| Tab::parse(&code))
        .unwrap_or(Tab::Account);
    let notice = query_param(&request.path, "msg").and_then(|code| Notice::parse(&code));
    let vault = context.vaults.get(None)?;
    let state = snapshot(context, &vault)?;
    let view = View {
        lang,
        tab,
        notice,
        state: &state,
    };
    Ok(page(render(&view), requested))
}
