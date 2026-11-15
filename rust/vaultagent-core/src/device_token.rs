//! Jeton d'appareil `va1.<uuid>.<base64url 32 octets>` : génération, lecture de l'en-tête `Authorization`,
//! identifiant UUID aléatoire.

use crate::encoding::{base64url_encode, digest, hex_encode};
use crate::entropy::Entropy;
use crate::error::Result;
use zeroize::Zeroizing;

const PREFIX: &str = "va1.";
const BEARER: &str = "Bearer ";
const SECRET_LEN: usize = 43;
const ID_LEN: usize = 36;

/// Jeton fraîchement émis, son identifiant d'appareil et son empreinte.
pub struct IssuedToken {
    /// Jeton complet, remis une seule fois au client.
    pub token: Zeroizing<String>,
    /// Identifiant de l'appareil (deuxième segment).
    pub device_id: String,
    /// Empreinte SHA-256 hexadécimale du jeton, seule partie conservée.
    pub hash: String,
}

/// Tire un UUID version 4 en minuscules.
///
/// Paramètres : la source d'aléa.
/// Retour : l'identifiant `8-4-4-4-12`.
/// # Errors
///
/// `engine-entropy` si l'aléa est indisponible.
pub fn random_uuid(entropy: &mut impl Entropy) -> Result<String> {
    let mut bytes = [0u8; 16];
    entropy.fill(&mut bytes)?;
    bytes[6] = bytes[6] & 0x0f | 0x40;
    bytes[8] = bytes[8] & 0x3f | 0x80;
    let hex = hex_encode(&bytes);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

/// Émet un jeton d'appareil.
///
/// Paramètres : la source d'aléa.
/// Retour : le jeton, l'identifiant et l'empreinte.
/// # Errors
///
/// `engine-entropy` si l'aléa est indisponible.
pub fn issue_token(entropy: &mut impl Entropy) -> Result<IssuedToken> {
    let device_id = random_uuid(entropy)?;
    let mut secret = Zeroizing::new([0u8; 32]);
    entropy.fill(secret.as_mut_slice())?;
    let token = Zeroizing::new(format!("{PREFIX}{device_id}.{}", base64url_encode(secret.as_slice())));
    let hash = digest(token.as_bytes());
    Ok(IssuedToken { token, device_id, hash })
}

/// Extrait le jeton d'un en-tête `Authorization` selon `^Bearer (va1\.[a-f0-9-]{36}\.[A-Za-z0-9_-]{43})$`.
///
/// Paramètres : l'en-tête, s'il est présent.
/// Retour : le jeton complet et son identifiant d'appareil, ou `None` si la forme est fausse.
#[must_use]
pub fn parse_bearer(header: Option<&str>) -> Option<(&str, &str)> {
    let token = header?.strip_prefix(BEARER)?;
    let rest = token.strip_prefix(PREFIX)?;
    let (id, secret) = rest.split_once('.')?;
    let id_ok = id.len() == ID_LEN && id.bytes().all(|b| matches!(b, b'a'..=b'f' | b'0'..=b'9' | b'-'));
    let secret_ok = secret.len() == SECRET_LEN
        && secret
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    (id_ok && secret_ok).then_some((token, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entropy::SeededEntropy;
    use crate::uuid::Uuid;

    #[test]
    fn issued_token_has_the_protocol_shape() {
        let issued = issue_token(&mut SeededEntropy::new(1)).unwrap();
        let header = format!("Bearer {}", issued.token.as_str());
        assert_eq!(
            parse_bearer(Some(&header)),
            Some((issued.token.as_str(), issued.device_id.as_str()))
        );
        assert_eq!(issued.hash, digest(issued.token.as_bytes()));
        assert!(Uuid::parse(&issued.device_id).is_some());
        assert_eq!(issued.device_id.as_bytes()[14], b'4');
        assert!(matches!(issued.device_id.as_bytes()[19], b'8' | b'9' | b'a' | b'b'));
        assert_eq!(issued.token.len(), 4 + 36 + 1 + 43);
    }

    #[test]
    fn bearer_parsing_is_strict() {
        let id = "0000000a-0000-4000-8000-00000000000b";
        let secret = "A".repeat(43);
        let good = format!("Bearer va1.{id}.{secret}");
        assert!(parse_bearer(Some(&good)).is_some());
        for bad in [
            format!("bearer va1.{id}.{secret}"),
            format!("Bearer  va1.{id}.{secret}"),
            format!("Bearer va2.{id}.{secret}"),
            format!("Bearer va1.{id}.{secret}\n"),
            format!("Bearer va1.{id}.{secret}x"),
            format!("Bearer va1.{id}.{}", "A".repeat(42)),
            format!("Bearer va1.{}.{secret}", id.to_uppercase()),
            format!("Bearer va1.{}.{secret}", &id[1..]),
            format!("Bearer va1.{id}.{}+", "A".repeat(42)),
            format!("va1.{id}.{secret}"),
        ] {
            assert!(parse_bearer(Some(&bad)).is_none(), "{bad}");
        }
        assert!(parse_bearer(None).is_none());
    }
}
