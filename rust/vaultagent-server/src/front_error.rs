//! Erreurs lisibles du front : un formulaire du navigateur reçoit une redirection `303` avec un code de message, une
//! page demandée par un navigateur reçoit une page HTML d'une phrase ; tout autre appelant garde le JSON du protocole.

use crate::error::ServerError;
use crate::front_lang::Lang;
use crate::front_notice::Notice;
use crate::front_response::page;
use crate::front_screen::Screen;
use crate::http::{Request, Response};
use crate::i18n::text;

const FORM_TYPE: &str = "application/x-www-form-urlencoded";
const ACTION_PREFIX: &str = "/admin/";
const KEPT_AS_JSON: [&str; 3] = ["proxy-required", "owner-required", "unauthorized"];

/// Indique si la requête est un formulaire du front : `POST /admin/<action>` (la route JSON est `/api/admin/*`)
/// déclarant le type de formulaire ou venant d'une navigation.
fn is_front_form(request: &Request) -> bool {
    let declared = request.header("content-type").unwrap_or_default().to_ascii_lowercase();
    request.method == "POST"
        && request.route_path().starts_with(ACTION_PREFIX)
        && (declared.starts_with(FORM_TYPE) || request.header("sec-fetch-mode") == Some("navigate"))
}

/// Indique si la requête est une page demandée par un navigateur : `GET` avec `Sec-Fetch-Mode: navigate` ou un
/// `Accept` contenant `text/html`.
fn is_browser_page(request: &Request) -> bool {
    request.method == "GET"
        && (request.header("sec-fetch-mode") == Some("navigate")
            || request
                .header("accept")
                .is_some_and(|accept| accept.to_ascii_lowercase().contains("text/html")))
}

/// Phrase de la page d'erreur.
fn sentence(code: &str) -> &'static str {
    match code {
        "origin-refused" | "route-refused" => "Page introuvable.",
        "proxy-required" | "owner-required" | "unauthorized" => "Accès refusé.",
        _ => Notice::from_error(code).label(),
    }
}

/// Page d'erreur HTML d'une phrase avec un lien de retour, aux en-têtes de sécurité du front.
fn error_page(request: &Request, error: &ServerError) -> Response {
    let lang = Lang::choose(None, request.header("cookie"));
    let body = format!(
        "<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
content=\"width=device-width,initial-scale=1\"><title>VaultAgent</title><link rel=\"stylesheet\" href=\"/style.css\"></head>\n\
<body><header class=\"bar\"><a class=\"brand\" href=\"/\"><span class=\"shield\">◆</span> VaultAgent</a></header>\n\
<main><section class=\"tab\"><p class=\"message\" role=\"alert\">{}</p><p><a href=\"/\">{}</a></p></section></main></body></html>\n",
        lang.code(),
        text(lang, sentence(error.code_text())),
        text(lang, "Retour"),
    );
    let mut response = page(body, None);
    response.status = Response::error(error).status;
    response
}

/// Réponse à une erreur de route.
///
/// Paramètres : la requête, l'erreur.
/// Retour : la redirection `303` vers la page pour un formulaire du front (sauf `proxy-required`, `owner-required`
/// et `unauthorized`), la page d'erreur pour une page de navigateur, sinon le JSON du protocole.
#[must_use]
pub fn reply(request: &Request, error: &ServerError) -> Response {
    let code = error.code_text();
    if is_front_form(request) && !KEPT_AS_JSON.contains(&code) {
        return crate::front_response::redirect(Screen::Main, Notice::from_error(code));
    }
    if is_browser_page(request) {
        return error_page(request, error);
    }
    Response::error(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vaultagent_core::ErrorCode;

    /// Erreur d'origine refusée des essais.
    fn refused() -> ServerError {
        ErrorCode::OriginRefused.into()
    }

    #[test]
    fn a_form_is_redirected_and_a_json_post_is_not() {
        let form = Request::new("POST", "/admin/lock", "127.0.0.1").with_header("content-type", FORM_TYPE);
        assert_eq!(reply(&form, &refused()).header("location"), Some("/?msg=stale-page"));
        let json = Request::new("POST", "/api/admin/lock", "127.0.0.1").with_header("content-type", FORM_TYPE);
        assert_eq!(reply(&json, &refused()).status, 403);
        let owner = ErrorCode::OwnerRequired.into();
        assert_eq!(reply(&form, &owner).status, 403);
    }

    #[test]
    fn a_page_error_is_html_only_for_a_browser() {
        let browser = Request::new("GET", "/nope", "127.0.0.1").with_header("accept", "text/html,*/*");
        let response = reply(&browser, &refused());
        assert_eq!(response.status, 403);
        assert!(String::from_utf8_lossy(&response.body).contains("href=\"/\""));
        let agent = Request::new("GET", "/nope", "127.0.0.1");
        assert_eq!(
            reply(&agent, &refused()).body_json().unwrap()["error"],
            "origin-refused"
        );
    }
}
