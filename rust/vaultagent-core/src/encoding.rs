//! Base64 standard et `base64url` sans remplissage, empreinte SHA-256 et comparaison à temps constant,
//! équivalents de `Buffer.toString`, `digest` et `same` de Node.

use base64::Engine as _;
use base64::alphabet;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// Encode en base64 standard avec remplissage.
///
/// Paramètres : les octets.
/// Retour : le texte.
#[must_use]
pub fn base64_encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// Encode en `base64url` sans remplissage.
///
/// Paramètres : les octets.
/// Retour : le texte.
#[must_use]
pub fn base64url_encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Décode du base64 standard, remplissage facultatif.
///
/// Paramètres : le texte.
/// Retour : les octets, ou `None` si le texte n'est pas du base64 valide.
#[must_use]
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    LENIENT.decode(text).ok()
}

/// Écrit des octets en hexadécimal minuscule.
///
/// Paramètres : les octets.
/// Retour : deux caractères par octet.
#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            text.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
            text.push(char::from(b"0123456789abcdef"[usize::from(byte & 15)]));
            text
        })
}

/// Empreinte SHA-256 en hexadécimal minuscule.
///
/// Paramètres : les octets à hacher.
/// Retour : 64 caractères hexadécimaux.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    hex_encode(&Sha256::digest(bytes))
}

/// Compare deux textes à temps constant ; des longueurs différentes sont refusées d'emblée, comme `same` de Node.
///
/// Paramètres : les deux textes.
/// Retour : `true` s'ils sont identiques.
#[must_use]
pub fn same(left: &str, right: &str) -> bool {
    left.len() == right.len() && bool::from(left.as_bytes().ct_eq(right.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_matches_known_sha256_vectors() {
        assert_eq!(
            digest(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn same_requires_equal_content() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "abcd"));
        assert!(same("", ""));
    }

    #[test]
    fn base64_round_trips_and_accepts_missing_padding() {
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_decode("YWI="), Some(b"ab".to_vec()));
        assert_eq!(base64_decode("YWI"), Some(b"ab".to_vec()));
        assert_eq!(base64_decode("***"), None);
        assert_eq!(base64url_encode(&[0xfb, 0xff]), "-_8");
    }
}
