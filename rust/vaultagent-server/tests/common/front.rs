//! Outils des essais du front : requêtes `GET` et formulaires `POST` du propriétaire, lecture des réponses HTML.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use vaultagent_server::{Request, Response};

use super::service::{ORIGIN, OWNER};

/// Requête `GET` du propriétaire.
pub fn get(path: &str) -> Request {
    Request::new("GET", path, "127.0.0.1").with_header("x-authentik-username", OWNER)
}

/// Requête `GET` du propriétaire avec un en-tête `cookie`.
pub fn get_with_cookie(path: &str, cookie: &str) -> Request {
    get(path).with_header("cookie", cookie)
}

/// Formulaire `POST /admin/<action>` avec l'origine et le propriétaire donnés (`None` : en-tête absent) et le type
/// de contenu donné.
pub fn form_as(action: &str, body: &str, origin: Option<&str>, owner: Option<&str>, kind: &str) -> Request {
    let mut request = Request::new("POST", &format!("/admin/{action}"), "127.0.0.1")
        .with_header("content-type", kind)
        .with_body(body.as_bytes());
    if let Some(value) = origin {
        request = request.with_header("origin", value);
    }
    if let Some(value) = owner {
        request = request.with_header("x-authentik-username", value);
    }
    request
}

/// Formulaire `POST /admin/<action>` du propriétaire, origine du service.
pub fn form(action: &str, body: &str) -> Request {
    form_as(
        action,
        body,
        Some(ORIGIN),
        Some(OWNER),
        "application/x-www-form-urlencoded",
    )
}

/// Corps de la réponse en texte.
pub fn html(response: &Response) -> String {
    String::from_utf8(response.body.clone()).unwrap()
}

/// En-tête `location` d'une redirection.
pub fn location(response: &Response) -> String {
    response.header("location").unwrap().to_owned()
}
