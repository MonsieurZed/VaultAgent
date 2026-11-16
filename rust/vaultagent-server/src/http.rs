//! Requête et réponse HTTP de la crate, indépendantes de toute bibliothèque de transport.

use serde::Serialize;
use serde_json::Value;
use vaultagent_core::ErrorCode;

use crate::error::ServerError;

/// Requête reçue : méthode, chemin avec requête éventuelle, en-têtes, adresse distante et corps en octets.
#[derive(Debug, Clone, Default)]
pub struct Request {
    /// Méthode HTTP en majuscules.
    pub method: String,
    /// Chemin de la cible, avec sa requête (`?a=b`) si elle existe.
    pub path: String,
    /// En-têtes dans l'ordre reçu.
    pub headers: Vec<(String, String)>,
    /// Adresse IP distante, éventuellement préfixée `::ffff:`.
    pub remote: String,
    /// Corps de la requête.
    pub body: Vec<u8>,
}

impl Request {
    /// Crée une requête sans en-tête ni corps.
    ///
    /// Paramètres : la méthode, le chemin, l'adresse distante.
    /// Retour : la requête.
    #[must_use]
    pub fn new(method: &str, path: &str, remote: &str) -> Self {
        Self {
            method: method.to_owned(),
            path: path.to_owned(),
            headers: Vec::new(),
            remote: remote.to_owned(),
            body: Vec::new(),
        }
    }

    /// Ajoute un en-tête.
    ///
    /// Paramètres : le nom, la valeur.
    /// Retour : la requête.
    #[must_use]
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_ascii_lowercase(), value.to_owned()));
        self
    }

    /// Remplace le corps.
    ///
    /// Paramètres : les octets du corps.
    /// Retour : la requête.
    #[must_use]
    pub fn with_body(mut self, body: &[u8]) -> Self {
        self.body = body.to_vec();
        self
    }

    /// Valeur du premier en-tête de ce nom, sans tenir compte de la casse.
    ///
    /// Paramètres : le nom de l'en-tête.
    /// Retour : la valeur, ou `None`.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Chemin sans la requête.
    ///
    /// Paramètres : aucun.
    /// Retour : la partie avant `?`.
    #[must_use]
    pub fn route_path(&self) -> &str {
        self.path.split_once('?').map_or(self.path.as_str(), |(path, _)| path)
    }
}

/// Réponse : statut, en-têtes et corps en octets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// Code de statut.
    pub status: u16,
    /// En-têtes dans l'ordre d'émission.
    pub headers: Vec<(&'static str, String)>,
    /// Corps de la réponse.
    pub body: Vec<u8>,
}

impl Response {
    /// Réponse JSON compacte avec les en-têtes communs du protocole.
    ///
    /// Paramètres : le statut, la valeur à sérialiser.
    /// Retour : la réponse ; une valeur non sérialisable donne `request-refused`.
    #[must_use]
    pub fn json<T: Serialize>(status: u16, value: &T) -> Self {
        match serde_json::to_vec(value) {
            Ok(body) => Self::with_body(status, body),
            Err(_) => Self::error(&ServerError::Code(ErrorCode::RequestRefused)),
        }
    }

    /// Réponse d'erreur `{"error": code}` ; le code hors de la forme du protocole devient `request-refused`.
    ///
    /// Paramètres : l'erreur.
    /// Retour : la réponse, de statut 403, 423 ou 400.
    #[must_use]
    pub fn error(error: &ServerError) -> Self {
        let text = error.code_text();
        let code = if well_formed(text) { text } else { "request-refused" };
        let status = match code {
            "unauthorized" | "owner-required" | "proxy-required" | "origin-refused" => 403,
            "locked" => 423,
            _ => 400,
        };
        let mut body = serde_json::Map::new();
        body.insert("error".to_owned(), Value::String(code.to_owned()));
        Self::with_body(status, Value::Object(body).to_string().into_bytes())
    }

    /// Corps lu comme JSON.
    ///
    /// Paramètres : aucun.
    /// Retour : la valeur, ou `None` si le corps n'est pas du JSON.
    #[must_use]
    pub fn body_json(&self) -> Option<Value> {
        serde_json::from_slice(&self.body).ok()
    }

    /// Valeur d'un en-tête de la réponse.
    ///
    /// Paramètres : le nom en minuscules.
    /// Retour : la valeur, ou `None`.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Construit la réponse JSON avec ses trois en-têtes communs.
    fn with_body(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: vec![
                ("content-type", "application/json".to_owned()),
                ("cache-control", "no-store".to_owned()),
                ("x-content-type-options", "nosniff".to_owned()),
            ],
            body,
        }
    }
}

/// Indique si un code suit `^[a-z-]+(?::[a-z-]+)?$`.
fn well_formed(code: &str) -> bool {
    let part = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'-');
    match code.split_once(':') {
        Some((stage, category)) => part(stage) && part(category),
        None => part(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_follow_the_protocol_status_table() {
        let status = |code: ErrorCode| Response::error(&code.into()).status;
        assert_eq!(status(ErrorCode::Unauthorized), 403);
        assert_eq!(status(ErrorCode::Locked), 423);
        assert_eq!(status(ErrorCode::InvalidWrite), 400);
        assert_eq!(
            Response::error(&ServerError::Io("x".into())).body_json(),
            serde_json::from_str("{\"error\":\"request-refused\"}").ok()
        );
    }

    #[test]
    fn headers_are_case_insensitive_and_query_is_cut() {
        let request = Request::new("GET", "/a?b=1", "127.0.0.1").with_header("Origin", "x");
        assert_eq!((request.header("ORIGIN"), request.route_path()), (Some("x"), "/a"));
        assert!(well_formed("sharing:refused") && !well_formed("Bad") && !well_formed("a:"));
    }
}
