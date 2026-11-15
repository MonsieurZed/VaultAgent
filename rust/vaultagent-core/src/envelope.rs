//! Enveloppe `seal` de `session.mjs` : clé AES-256 aléatoire chiffrée en RSA-OAEP SHA-256 pour le client, valeur
//! chiffrée en AES-256-GCM ; et son ouverture côté client.

use crate::approver_key::parse_spki;
use crate::encoding::{base64_decode, base64_encode};
use crate::entropy::{Entropy, EntropyRng};
use crate::error::{ErrorCode, Result};
use crate::protect::{KEY_LEN, NONCE_LEN, TAG_LEN, open_parts};
use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rsa::{Oaep, RsaPrivateKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

const MIN_BITS: usize = 2048;
const MAX_BITS: usize = 4096;

/// Enveloppe remise au client ; champs base64, noms du protocole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    /// Clé AES chiffrée en RSA-OAEP SHA-256.
    pub encrypted_key: String,
    /// Nonce AES-GCM de 12 octets.
    pub nonce: String,
    /// Étiquette AES-GCM de 16 octets.
    pub tag: String,
    /// Valeur chiffrée.
    pub data: String,
}

/// Chiffre une valeur pour le propriétaire d'une clé publique RSA.
///
/// Paramètres : la valeur en clair, la clé SPKI en base64 (RSA de 2048 à 4096 bits), la source d'aléa.
/// Retour : l'enveloppe.
/// # Errors
///
/// `invalid-key` si la clé est illisible, non RSA ou hors plage ; `engine-entropy` si l'aléa est indisponible.
pub fn seal(value: &[u8], spki_base64: &str, entropy: &mut impl Entropy) -> Result<Envelope> {
    let key = parse_spki(spki_base64, MIN_BITS, MAX_BITS)?;
    let mut aes = Zeroizing::new([0u8; KEY_LEN]);
    entropy.fill(aes.as_mut_slice())?;
    let mut nonce = [0u8; NONCE_LEN];
    entropy.fill(&mut nonce)?;
    let cipher = Aes256Gcm::new_from_slice(aes.as_slice()).map_err(|_| ErrorCode::InvalidKey)?;
    let mut data = Zeroizing::new(value.to_vec());
    let tag = cipher
        .encrypt_in_place_detached(Nonce::from_slice(&nonce), b"", &mut data)
        .map_err(|_| ErrorCode::InvalidKey)?;
    let mut rng = EntropyRng::new(entropy);
    let wrapped = key.encrypt(&mut rng, Oaep::new::<Sha256>(), aes.as_slice());
    rng.finish()?;
    Ok(Envelope {
        encrypted_key: base64_encode(&wrapped.map_err(|_| ErrorCode::InvalidKey)?),
        nonce: base64_encode(&nonce),
        tag: base64_encode(&tag),
        data: base64_encode(&data),
    })
}

/// Ouvre une enveloppe avec la clé privée du client.
///
/// Paramètres : l'enveloppe, la clé privée RSA, la source d'aléa (aveuglement du déchiffrement RSA).
/// Retour : la valeur en clair, effacée à la destruction.
/// # Errors
///
/// `invalid-key` si un champ est mal formé, si la clé ne convient pas ou si l'authentification échoue ;
/// `engine-entropy` si l'aléa est indisponible.
pub fn open_envelope(
    envelope: &Envelope,
    private_key: &RsaPrivateKey,
    entropy: &mut impl Entropy,
) -> Result<Zeroizing<Vec<u8>>> {
    let wrapped = base64_decode(&envelope.encrypted_key).ok_or(ErrorCode::InvalidKey)?;
    let mut rng = EntropyRng::new(entropy);
    let aes = private_key.decrypt_blinded(&mut rng, Oaep::new::<Sha256>(), &wrapped);
    rng.finish()?;
    let aes = Zeroizing::new(aes.map_err(|_| ErrorCode::InvalidKey)?);
    let cipher = Aes256Gcm::new_from_slice(aes.as_slice()).map_err(|_| ErrorCode::InvalidKey)?;
    let nonce = base64_decode(&envelope.nonce).filter(|bytes| bytes.len() == NONCE_LEN);
    let tag = base64_decode(&envelope.tag).filter(|bytes| bytes.len() == TAG_LEN);
    let (Some(nonce), Some(tag), Some(data)) = (nonce, tag, base64_decode(&envelope.data)) else {
        return Err(ErrorCode::InvalidKey);
    };
    open_parts(&cipher, &nonce, &tag, data)
}
