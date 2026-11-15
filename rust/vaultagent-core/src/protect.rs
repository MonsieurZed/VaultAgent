//! Chiffrement AES-256-GCM du magasin (`protect` et `unprotect` de `store.mjs`) : format `{nonce, tag, data}` en base64,
//! nonce de 96 bits, étiquette de 128 bits.

use crate::encoding::{base64_decode, base64_encode};
use crate::entropy::Entropy;
use crate::error::{ErrorCode, Result};
use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce, Tag};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Taille de la clé machine en octets.
pub const KEY_LEN: usize = 32;
pub(crate) const NONCE_LEN: usize = 12;
pub(crate) const TAG_LEN: usize = 16;

/// Valeur chiffrée telle qu'écrite dans les fichiers du magasin : trois champs base64.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Protected {
    /// Nonce de 12 octets.
    pub nonce: String,
    /// Étiquette d'authentification de 16 octets.
    pub tag: String,
    /// Texte chiffré.
    pub data: String,
}

/// Chiffre une valeur avec la clé machine et un nonce tiré de la source d'aléa.
///
/// Paramètres : la clé de 32 octets, la valeur en clair, la source d'aléa.
/// Retour : la valeur chiffrée.
/// # Errors
///
/// `engine-entropy` si l'aléa est indisponible ; `invalid-key` si la clé n'a pas 32 octets.
pub fn protect(key: &[u8], value: &[u8], entropy: &mut impl Entropy) -> Result<Protected> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| ErrorCode::InvalidKey)?;
    let mut nonce = [0u8; NONCE_LEN];
    entropy.fill(&mut nonce)?;
    let mut data = value.to_vec();
    let tag = cipher
        .encrypt_in_place_detached(Nonce::from_slice(&nonce), b"", &mut data)
        .map_err(|_| ErrorCode::InvalidKey)?;
    Ok(Protected {
        nonce: base64_encode(&nonce),
        tag: base64_encode(&tag),
        data: base64_encode(&data),
    })
}

/// Déchiffre et authentifie une valeur du magasin.
///
/// Paramètres : la clé de 32 octets, la valeur chiffrée.
/// Retour : le texte en clair, effacé à la destruction.
/// # Errors
///
/// `invalid-key` si la clé est inadaptée, si un champ est mal formé ou si l'étiquette ne correspond pas
/// (mauvaise clé ou donnée altérée).
pub fn unprotect(key: &[u8], value: &Protected) -> Result<Zeroizing<Vec<u8>>> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| ErrorCode::InvalidKey)?;
    let nonce = base64_decode(&value.nonce).filter(|bytes| bytes.len() == NONCE_LEN);
    let tag = base64_decode(&value.tag).filter(|bytes| bytes.len() == TAG_LEN);
    let data = base64_decode(&value.data);
    let (Some(nonce), Some(tag), Some(data)) = (nonce, tag, data) else {
        return Err(ErrorCode::InvalidKey);
    };
    open_parts(&cipher, &nonce, &tag, data)
}

/// Authentifie et déchiffre trois morceaux déjà décodés.
///
/// Paramètres : le chiffre, le nonce de 12 octets, l'étiquette de 16 octets, le texte chiffré.
/// Retour : le texte en clair, effacé à la destruction.
/// # Errors
///
/// `invalid-key` si l'authentification échoue.
pub(crate) fn open_parts(cipher: &Aes256Gcm, nonce: &[u8], tag: &[u8], data: Vec<u8>) -> Result<Zeroizing<Vec<u8>>> {
    let mut clear = Zeroizing::new(data);
    cipher
        .decrypt_in_place_detached(Nonce::from_slice(nonce), b"", &mut clear, Tag::from_slice(tag))
        .map_err(|_| ErrorCode::InvalidKey)?;
    Ok(clear)
}
