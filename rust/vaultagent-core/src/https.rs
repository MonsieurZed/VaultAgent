//! Adresses HTTPS : origine d'une URI de fiche (`originOf` de Node) et adresse acceptée à l'écriture d'une fiche.
//!
//! Lecture volontairement plus stricte que WHATWG : ni espace ni caractère de contrôle, ni `@` dans l'autorité,
//! hôte ASCII (les noms internationalisés sont refusés), schéma `https` obligatoire suivi de `//`.

use crate::jsval::utf16_len;

/// Longueur maximale d'une adresse de fiche.
pub const MAX_ADDRESS_LENGTH: usize = 2048;

/// Éléments utiles d'une URL `https:` lue.
struct Parsed {
    host: String,
    port: Option<u16>,
    query: bool,
    fragment: bool,
}

/// Indique si un hôte est un nom ASCII simple ou une adresse IPv6 entre crochets.
fn valid_host(host: &str) -> bool {
    match host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')) {
        Some(inner) => !inner.is_empty() && inner.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.'),
        None => {
            !host.is_empty()
                && host
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
        }
    }
}

/// Sépare l'hôte du port dans l'autorité.
fn split_authority(authority: &str) -> Option<(&str, &str)> {
    if authority.starts_with('[') {
        let end = authority.find(']')? + 1;
        let (host, rest) = authority.split_at(end);
        let port = if rest.is_empty() {
            Some("")
        } else {
            rest.strip_prefix(':')
        };
        return port.map(|port| (host, port));
    }
    Some(authority.split_once(':').unwrap_or((authority, "")))
}

/// Lit un port : vide (absent), ou chiffres jusqu'à 65535 ; 443 est le port par défaut et disparaît.
fn parse_port(text: &str) -> Result<Option<u16>, ()> {
    if text.is_empty() {
        return Ok(None);
    }
    let digits_only = text.len() <= 5 && text.bytes().all(|b| b.is_ascii_digit());
    let port = digits_only.then(|| text.parse::<u16>().ok()).flatten().ok_or(())?;
    Ok((port != 443).then_some(port))
}

/// Lit une URL `https:` sans identifiants.
fn parse(text: &str) -> Option<Parsed> {
    if text.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let (scheme, rest) = text.split_once(':')?;
    let rest = scheme
        .eq_ignore_ascii_case("https")
        .then(|| rest.strip_prefix("//"))
        .flatten()?;
    let end = rest.find(['/', '?', '#', '\\']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.contains('@') {
        return None;
    }
    let (host, port_text) = split_authority(authority)?;
    valid_host(host).then_some(())?;
    let (before_fragment, fragment) = tail.split_once('#').unwrap_or((tail, ""));
    let query = before_fragment.split_once('?').map_or("", |(_, query)| query);
    Some(Parsed {
        host: host.to_ascii_lowercase(),
        port: parse_port(port_text).ok()?,
        query: !query.is_empty(),
        fragment: !fragment.is_empty(),
    })
}

/// Origine `https:` d'une URI, sans identifiants.
///
/// Paramètres : le texte de l'URI.
/// Retour : `https://hôte[:port]` en minuscules sans port par défaut, ou `None` si l'URI n'est pas `https:` valide
/// ou porte des identifiants.
#[must_use]
pub fn https_origin(text: &str) -> Option<String> {
    let parsed = parse(text)?;
    Some(match parsed.port {
        Some(port) => format!("https://{}:{port}", parsed.host),
        None => format!("https://{}", parsed.host),
    })
}

/// Adresse HTTPS acceptée dans une fiche : sans identifiants, requête ni fragment, 2048 unités UTF-16 au plus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpsAddress(String);

impl HttpsAddress {
    /// Valide une adresse de fiche.
    ///
    /// Paramètres : le texte proposé.
    /// Retour : l'adresse (texte d'origine conservé), ou `None` si elle est vide, trop longue, non `https:`,
    /// ou porte identifiants, requête ou fragment.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let sized = !text.is_empty() && utf16_len(text) <= MAX_ADDRESS_LENGTH;
        let parsed = sized.then(|| parse(text)).flatten()?;
        (!parsed.query && !parsed.fragment).then(|| Self(text.to_owned()))
    }

    /// Texte de l'adresse.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_normalizes_host_and_port() {
        assert_eq!(
            https_origin("https://Example.TEST/login?a=1#x").as_deref(),
            Some("https://example.test")
        );
        assert_eq!(
            https_origin("https://example.test:443/").as_deref(),
            Some("https://example.test")
        );
        assert_eq!(
            https_origin("HTTPS://example.test:8443").as_deref(),
            Some("https://example.test:8443")
        );
        assert_eq!(
            https_origin("https://[::1]:9000/x").as_deref(),
            Some("https://[::1]:9000")
        );
        assert_eq!(
            https_origin("https://example.test:/").as_deref(),
            Some("https://example.test")
        );
    }

    #[test]
    fn origin_refuses_other_forms() {
        for bad in [
            "http://example.test",
            "https://user:pass@example.test",
            "https://user@example.test",
            "example.test",
            "https:example.test",
            "https://",
            "https://exa mple.test",
            "https://example.test:99999",
            "https://example.test:ab",
            "https://exämple.test",
            "ftp://example.test",
            "",
            "https://[::1",
            "https://[::1]x",
        ] {
            assert_eq!(https_origin(bad), None, "{bad}");
        }
    }

    #[test]
    fn address_accepts_plain_https() {
        assert!(HttpsAddress::parse("https://example.test").is_some());
        assert!(HttpsAddress::parse("https://example.test/login").is_some());
        assert_eq!(
            HttpsAddress::parse("https://example.test/?")
                .map(|a| a.as_str().to_owned())
                .as_deref(),
            Some("https://example.test/?")
        );
    }

    #[test]
    fn address_refuses_query_fragment_credentials_and_size() {
        for bad in [
            "http://nvr.example.test",
            "https://u:p@nvr.example.test",
            "https://nvr.example.test/?a=1",
            "https://nvr.example.test/#x",
            "nvr",
            "",
        ] {
            assert!(HttpsAddress::parse(bad).is_none(), "{bad}");
        }
        let long = format!("https://example.test/{}", "a".repeat(2048));
        assert!(HttpsAddress::parse(&long).is_none());
        let exact = format!(
            "https://example.test/{}",
            "a".repeat(2048 - "https://example.test/".len())
        );
        assert!(HttpsAddress::parse(&exact).is_some());
    }
}
