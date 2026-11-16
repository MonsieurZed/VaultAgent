//! Lecture d'une requête `tiny_http` vers la [`Request`] du cœur HTTP : en-têtes bornés, corps borné.

use std::io::Read;

use crate::http::Request;

/// En-têtes acceptés au plus (`server.maxHeadersCount` de Node).
pub const MAX_HEADERS: usize = 40;
const BODY_LIMIT: u64 = 24_576;
const HEADERS_TOO_LARGE: u16 = 431;
const BAD_REQUEST: u16 = 400;

/// Convertit une requête reçue.
///
/// Le corps est lu jusqu'à une octet de plus que la limite du protocole : `route` rend `request-size` sans que la
/// mémoire dépende du client.
///
/// Paramètres : la requête `tiny_http`.
/// Retour : la requête du cœur HTTP, ou le statut à rendre (431 si plus de 40 en-têtes, 400 si le corps ne se lit pas).
pub fn read(incoming: &mut tiny_http::Request) -> Result<Request, u16> {
    if incoming.headers().len() > MAX_HEADERS {
        return Err(HEADERS_TOO_LARGE);
    }
    let remote = incoming.remote_addr().map(|address| address.ip().to_string());
    let mut request = Request::new(
        incoming.method().as_str(),
        incoming.url(),
        remote.as_deref().unwrap_or_default(),
    );
    for header in incoming.headers() {
        request = request.with_header(header.field.as_str().as_str(), header.value.as_str());
    }
    let mut body = Vec::new();
    incoming
        .as_reader()
        .take(BODY_LIMIT + 1)
        .read_to_end(&mut body)
        .map_err(|_| BAD_REQUEST)?;
    Ok(request.with_body(&body))
}
