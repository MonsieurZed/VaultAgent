//! Sonde de santé du binaire (`vaultagent-server healthcheck`) : un `GET /healthz` local, sans client HTTP externe.
//!
//! L'image d'exécution n'a ni shell ni `curl` : le binaire se sonde lui-même, avec la bibliothèque standard.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(3);
const REQUEST: &[u8] = b"GET /healthz HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
const MAX_REPLY: u64 = 4096;

/// Interroge `/healthz` sur `127.0.0.1`.
///
/// Paramètres : le port d'écoute du service.
/// Retour : `true` si la réponse porte le statut 200, `false` sinon (connexion refusée, délai de 3 s, autre statut).
#[must_use]
pub fn probe(port: u16) -> bool {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, TIMEOUT) else {
        return false;
    };
    if stream.set_read_timeout(Some(TIMEOUT)).is_err() || stream.set_write_timeout(Some(TIMEOUT)).is_err() {
        return false;
    }
    if stream.write_all(REQUEST).is_err() {
        return false;
    }
    let mut reply = Vec::new();
    if stream.take(MAX_REPLY).read_to_end(&mut reply).is_err() && reply.is_empty() {
        return false;
    }
    status_is_ok(&reply)
}

/// Lit le statut de la ligne de réponse.
///
/// Paramètres : les premiers octets de la réponse.
/// Retour : `true` si la première ligne est `HTTP/1.x 200`.
fn status_is_ok(reply: &[u8]) -> bool {
    let line = reply
        .split(|byte| *byte == b'\r' || *byte == b'\n')
        .next()
        .unwrap_or_default();
    let text = String::from_utf8_lossy(line);
    let mut parts = text.split_whitespace();
    parts.next().is_some_and(|version| version.starts_with("HTTP/1.")) && parts.next() == Some("200")
}

/// Lit le port d'écoute depuis `VAULTAGENT_PORT`.
///
/// Paramètres : la valeur de la variable, absente ou non.
/// Retour : le port, 8080 si la variable est absente, vide ou invalide.
#[must_use]
pub fn port_from(value: Option<&str>) -> u16 {
    value.and_then(|text| text.parse().ok()).unwrap_or(8080)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    /// Serveur à usage unique qui répond le texte donné.
    fn one_shot(reply: &'static [u8]) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 256];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(reply);
            }
        });
        port
    }

    #[test]
    fn status_200_is_healthy() {
        assert!(probe(one_shot(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")));
    }

    #[test]
    fn other_status_is_unhealthy() {
        assert!(!probe(one_shot(b"HTTP/1.1 503 Service Unavailable\r\n\r\n")));
    }

    #[test]
    fn garbage_is_unhealthy() {
        assert!(!probe(one_shot(b"pas du HTTP")));
    }

    #[test]
    fn closed_port_is_unhealthy() {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        assert!(!probe(port));
    }

    #[test]
    fn port_defaults_to_8080() {
        assert_eq!(port_from(None), 8080);
        assert_eq!(port_from(Some("")), 8080);
        assert_eq!(port_from(Some("abc")), 8080);
        assert_eq!(port_from(Some("9090")), 9090);
    }
}
