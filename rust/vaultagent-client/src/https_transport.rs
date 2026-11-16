//! Transport HTTPS réel sur `ureq` 3 : magasin de certificats du système, aucune redirection, réponse bornée.

use std::time::Duration;

use ureq::tls::{RootCerts, TlsConfig};
use ureq::{Agent, Error, Timeout};

use crate::server_address::ServerAddress;
use crate::transport::{CONNECT_TIMEOUT, MAX_RESPONSE_BYTES, Response, Transport, TransportError};

/// Client HTTPS lié à un serveur.
pub struct HttpsTransport {
    address: ServerAddress,
    agent: Agent,
}

impl HttpsTransport {
    /// Construit le client : certificats de la plateforme, zéro redirection, statuts non convertis en erreurs.
    ///
    /// Paramètres : l'adresse validée du serveur.
    /// Retour : le transport.
    #[must_use]
    pub fn new(address: ServerAddress) -> Self {
        let config = Agent::config_builder()
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
            .build();
        Self {
            address,
            agent: config.into(),
        }
    }
}

/// Indique si une erreur d'entrée-sortie prouve que la connexion n'a jamais été établie.
fn refused_before_sending(error: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        error.kind(),
        ErrorKind::ConnectionRefused
            | ErrorKind::AddrNotAvailable
            | ErrorKind::NetworkUnreachable
            | ErrorKind::HostUnreachable
            | ErrorKind::NotFound
    )
}

/// Classe une erreur `ureq` selon que la requête a pu partir.
fn classify(error: &Error) -> TransportError {
    match error {
        Error::BodyExceedsLimit(_) => TransportError::TooLarge,
        Error::TooManyRedirects | Error::RedirectFailed => TransportError::Redirected,
        Error::HostNotFound
        | Error::ConnectionFailed
        | Error::BadUri(_)
        | Error::Tls(_)
        | Error::Rustls(_)
        | Error::Pem(_)
        | Error::Timeout(Timeout::Resolve | Timeout::Connect) => {
            TransportError::NotSent("Cannot reach the server.".to_owned())
        }
        Error::Io(io) if refused_before_sending(io) => TransportError::NotSent("Cannot reach the server.".to_owned()),
        _ => TransportError::Indeterminate,
    }
}

impl Transport for HttpsTransport {
    /// Envoie le `POST` en HTTPS et lit au plus 4 Mio de réponse.
    fn post(
        &self,
        path: &str,
        bearer: Option<&str>,
        body: &[u8],
        timeout: Duration,
    ) -> Result<Response, TransportError> {
        let mut request = self
            .agent
            .post(self.address.url(path))
            .config()
            .timeout_global(Some(timeout))
            .build()
            .header("content-type", "application/json");
        if let Some(value) = bearer {
            request = request.header("authorization", value);
        }
        let mut response = request.send(body).map_err(|error| classify(&error))?;
        let status = response.status().as_u16();
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(|error| classify(&error))?;
        Ok(Response { status, body: bytes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn refused_connection_is_not_sent_and_a_reset_is_indeterminate() {
        let refused = Error::Io(IoError::from(ErrorKind::ConnectionRefused));
        assert!(matches!(classify(&refused), TransportError::NotSent(_)));
        let reset = Error::Io(IoError::from(ErrorKind::ConnectionReset));
        assert_eq!(classify(&reset), TransportError::Indeterminate);
    }
}
