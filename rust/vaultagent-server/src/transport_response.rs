//! Écriture d'une [`Response`] du cœur HTTP en réponse `tiny_http`, sans en-tête ajouté par la bibliothèque autre
//! que ceux du protocole HTTP lui-même (`Date`, `Server`, `Content-Length`).

use std::io::Cursor;

use tiny_http::{Header, StatusCode};

use crate::http::Response;

/// Convertit la réponse.
///
/// Paramètres : la réponse du cœur HTTP.
/// Retour : la réponse `tiny_http` ; un en-tête dont le texte n'est pas transmissible est omis.
#[must_use]
pub fn convert(response: Response) -> tiny_http::Response<Cursor<Vec<u8>>> {
    let headers = response
        .headers
        .iter()
        .filter_map(|(name, value)| Header::from_bytes(name.as_bytes(), value.as_bytes()).ok())
        .collect();
    let length = response.body.len();
    tiny_http::Response::new(
        StatusCode(response.status),
        headers,
        Cursor::new(response.body),
        Some(length),
        None,
    )
}

/// Réponse vide d'un statut donné, pour les requêtes refusées avant le routage.
///
/// Paramètres : le statut.
/// Retour : la réponse.
#[must_use]
pub fn bare(status: u16) -> tiny_http::Response<std::io::Empty> {
    tiny_http::Response::empty(StatusCode(status))
}
