//! `POST /admin/<action>` : formulaires du front. Contrôle de l'origine, lecture du formulaire, puis appel de la même
//! fonction que la route JSON ; la réponse est une redirection `303` portant un code de message.

use serde_json::Value;
use vaultagent_core::ErrorCode;

use crate::admin::act;
use crate::context::Context;
use crate::error::Result;
use crate::form::Form;
use crate::front_body::{Prepared, prepare};
use crate::front_lang::Lang;
use crate::front_notice::Notice;
use crate::front_pairing::pairing_page;
use crate::front_response::redirect;
use crate::http::{Request, Response};

const FORM_TYPE: &str = "application/x-www-form-urlencoded";
const BODY_LIMIT: usize = 24_576;
const ACTION_PREFIX: &str = "/admin/";

/// Corps du formulaire, après contrôle du type déclaré et de la taille.
fn read_form(request: &Request) -> Result<Form> {
    let declared = request.header("content-type").unwrap_or_default().to_ascii_lowercase();
    let is_form = declared
        .strip_prefix(FORM_TYPE)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(';'));
    if !is_form {
        return Err(ErrorCode::RequestRefused.into());
    }
    if request.body.len() > BODY_LIMIT {
        return Err(ErrorCode::RequestSize.into());
    }
    Ok(Form::parse(&request.body))
}

/// Réponse à `pairing` : la page du code, ou la redirection d'échec.
fn pairing_reply(context: &Context, request: &Request, reply: &Response, prepared: &Prepared) -> Response {
    let body = reply.body_json().unwrap_or(Value::Null);
    let code = body.get("code").and_then(Value::as_str);
    let seconds = body.get("expiresInSeconds").and_then(Value::as_u64);
    match (code, seconds) {
        (Some(code), Some(seconds)) => {
            let lang = Lang::choose(None, request.header("cookie"));
            pairing_page(lang, code, context.clock.wall_ms() + seconds * 1000)
        }
        _ => redirect(prepared.tab, Notice::Refused),
    }
}

/// Traite un formulaire d'administration.
///
/// Paramètres : le contexte, la requête (proxy et propriétaire déjà contrôlés), le chemin `/admin/<action>`.
/// Retour : la redirection `303` vers `/?tab=…&msg=<code>`, ou la page du code pour `pairing`.
///
/// # Errors
///
/// `origin-refused` si l'en-tête `origin` n'est pas exactement l'origine du service ; `request-refused` ou
/// `request-size` pour le corps ; `route-refused` pour une action inconnue.
pub fn handle(context: &Context, request: &Request, path: &str) -> Result<Response> {
    if request.method != "POST" || request.header("origin") != Some(context.origin.as_str()) {
        return Err(ErrorCode::OriginRefused.into());
    }
    let name = path.strip_prefix(ACTION_PREFIX).unwrap_or_default();
    let form = read_form(request)?;
    let prepared = prepare(name, &form).ok_or(ErrorCode::RouteRefused)?;
    match act(context, prepared.action, &prepared.body) {
        Ok(reply) if prepared.action == "pairing" => Ok(pairing_reply(context, request, &reply, &prepared)),
        Ok(_) => Ok(redirect(prepared.tab, Notice::Saved)),
        Err(error) => Ok(redirect(prepared.tab, Notice::from_error(error.code_text()))),
    }
}
