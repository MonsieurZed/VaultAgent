//! Client HTTP/1.1 minimal en clair sur `TcpStream` : une requête, une connexion, lecture jusqu'à la fermeture.

use crate::error::{ParityError, Result};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const READ_TIMEOUT_SECONDS: u64 = 120;

/// Requête à envoyer.
#[derive(Debug, Clone)]
pub struct Request {
    /// Méthode HTTP.
    pub method: String,
    /// Chemin avec sa requête éventuelle.
    pub path: String,
    /// En-têtes ajoutés à `Host`, `Connection` et `Content-Length`.
    pub headers: Vec<(String, String)>,
    /// Corps brut, absent pour un `GET`.
    pub body: Option<String>,
}

/// Réponse reçue ; les noms d'en-têtes sont en minuscules.
#[derive(Debug, Clone)]
pub struct Reply {
    /// Code de statut.
    pub status: u16,
    /// En-têtes dans l'ordre reçu.
    pub headers: Vec<(String, String)>,
    /// Corps décodé.
    pub body: String,
}

/// Écrit la requête sous forme de texte HTTP/1.1.
///
/// Paramètres : l'adresse `hôte:port` et la requête.
/// Retour : les octets à envoyer.
fn render(address: &str, request: &Request) -> Vec<u8> {
    let mut lines = vec![
        format!("{} {} HTTP/1.1", request.method, request.path),
        format!("Host: {address}"),
        "Connection: close".to_owned(),
    ];
    lines.extend(request.headers.iter().map(|(name, value)| format!("{name}: {value}")));
    if let Some(body) = &request.body {
        lines.push(format!("Content-Length: {}", body.len()));
    }
    let mut bytes = format!("{}\r\n\r\n", lines.join("\r\n")).into_bytes();
    if let Some(body) = &request.body {
        bytes.extend_from_slice(body.as_bytes());
    }
    bytes
}

/// Décode un corps `Transfer-Encoding: chunked`.
///
/// Paramètres : les octets bruts après les en-têtes.
/// Retour : le corps assemblé.
fn decode_chunked(raw: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    let mut rest = raw;
    while let Some(line_end) = rest.windows(2).position(|pair| pair == b"\r\n") {
        let size_text = String::from_utf8_lossy(&rest[..line_end]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("0").trim(), 16).unwrap_or(0);
        rest = &rest[line_end + 2..];
        if size == 0 || rest.len() < size {
            break;
        }
        body.extend_from_slice(&rest[..size]);
        rest = rest.get(size + 2..).unwrap_or(&[]);
    }
    body
}

/// Analyse la réponse brute.
///
/// Paramètres : tous les octets lus.
/// Retour : la réponse structurée.
/// # Errors
///
/// `http` si la ligne de statut ou la séparation des en-têtes manque.
pub fn parse(raw: &[u8]) -> Result<Reply> {
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| ParityError::Http("fin des en-têtes absente".to_owned()))?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| ParityError::Http("ligne de statut illisible".to_owned()))?;
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let chunked = headers
        .iter()
        .any(|(name, value)| name == "transfer-encoding" && value.contains("chunked"));
    let payload = &raw[split + 4..];
    let bytes = if chunked {
        decode_chunked(payload)
    } else {
        payload.to_vec()
    };
    Ok(Reply {
        status,
        headers,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

/// Envoie une requête et lit la réponse complète.
///
/// Paramètres : l'adresse `hôte:port` et la requête.
/// Retour : la réponse.
/// # Errors
///
/// `réseau` si la connexion, l'écriture ou la lecture échoue ; `http` si la réponse est illisible.
pub fn send(address: &str, request: &Request) -> Result<Reply> {
    let mut stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(Duration::from_secs(READ_TIMEOUT_SECONDS)))?;
    stream.write_all(&render(address, request))?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    parse(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_reply() {
        let raw = b"HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\n\r\n{\"error\":\"x\"}";
        let reply = parse(raw).unwrap();
        assert_eq!(reply.status, 403);
        assert_eq!(
            reply.headers,
            vec![("content-type".to_owned(), "application/json".to_owned())]
        );
        assert_eq!(reply.body, "{\"error\":\"x\"}");
    }

    #[test]
    fn parses_chunked_reply() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n";
        assert_eq!(parse(raw).unwrap().body, "abcde");
    }

    #[test]
    fn refuses_truncated_reply() {
        assert!(parse(b"HTTP/1.1 200 OK\r\n").is_err());
    }

    #[test]
    fn renders_content_length() {
        let request = Request {
            method: "POST".to_owned(),
            path: "/x".to_owned(),
            headers: vec![],
            body: Some("{}".to_owned()),
        };
        let text = String::from_utf8(render("h:1", &request)).unwrap();
        assert!(text.contains("Content-Length: 2\r\n\r\n{}"));
    }
}
