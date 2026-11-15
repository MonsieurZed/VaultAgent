//! Clés publiques RSA des PC approbateurs (`keys.mjs`) : lecture SPKI base64, vérification RSA-PSS SHA-256 à sel de
//! 32 octets et message de décision `VaultAgent.Unlock.v2`.

use crate::encoding::base64_decode;
use crate::error::{ErrorCode, Result};
use rsa::RsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::pss::{Signature, VerifyingKey};
use rsa::signature::Verifier;
use rsa::traits::PublicKeyParts;
use sha2::Sha256;

const MAX_ENCODED_LEN: usize = 1024;
const APPROVER_BITS: usize = 2048;
const SALT_LEN: usize = 32;
const MESSAGE_TAG: &str = "VaultAgent.Unlock.v2";

/// Lit une clé publique RSA encodée en SPKI DER puis base64 et contrôle la taille de son module.
///
/// Paramètres : le texte base64, les tailles minimale et maximale du module en bits.
/// Retour : la clé.
/// # Errors
///
/// `invalid-key` si le texte n'est pas du SPKI RSA valide ou si le module sort de la plage.
pub(crate) fn parse_spki(text: &str, min_bits: usize, max_bits: usize) -> Result<RsaPublicKey> {
    let der = base64_decode(text).ok_or(ErrorCode::InvalidKey)?;
    let key = RsaPublicKey::from_public_key_der(&der).map_err(|_| ErrorCode::InvalidKey)?;
    let bits = key.n().bits();
    if bits < min_bits || bits > max_bits {
        return Err(ErrorCode::InvalidKey);
    }
    Ok(key)
}

/// Charge une clé publique d'approbateur : RSA de 2048 bits exactement, 1024 caractères base64 au plus.
///
/// Paramètres : la clé SPKI en base64.
/// Retour : la clé chargée.
/// # Errors
///
/// `invalid-key` pour un texte trop long, illisible, non RSA ou d'une autre taille.
pub fn rsa_public_key(spki_base64: &str) -> Result<RsaPublicKey> {
    if spki_base64.len() > MAX_ENCODED_LEN {
        return Err(ErrorCode::InvalidKey);
    }
    parse_spki(spki_base64, APPROVER_BITS, APPROVER_BITS)
}

/// Vérifie une signature RSA-PSS SHA-256 de sel 32 octets.
///
/// Paramètres : la clé d'approbateur (SPKI base64), le message signé, la signature en base64, le code d'erreur à
/// rendre si la signature est refusée.
/// Retour : rien.
/// # Errors
///
/// `failure` si la signature est absente, mal encodée ou fausse ; `invalid-key` si la clé est refusée.
pub fn verify_pss(public_key: &str, message: &str, signature_base64: &str, failure: ErrorCode) -> Result<()> {
    let key = rsa_public_key(public_key)?;
    let signature = base64_decode(signature_base64)
        .and_then(|bytes| Signature::try_from(bytes.as_slice()).ok())
        .ok_or_else(|| failure.clone())?;
    VerifyingKey::<Sha256>::new_with_salt_len(key, SALT_LEN)
        .verify(message.as_bytes(), &signature)
        .map_err(|_| failure)
}

/// Construit le message signé par le PC approbateur : le `JSON.stringify` du tableau
/// `["VaultAgent.Unlock.v2", origin, vaultId, requestId, challenge, approve, remember]`.
///
/// Paramètres : l'origine du service, le coffre, la demande (`enroll-hello`, `enroll-elevation`, `reset-auto` ou un
/// identifiant), le défi, la décision et le souhait de mémoriser.
/// Retour : le JSON compact.
#[must_use]
pub fn decision_message(
    origin: &str,
    vault_id: &str,
    request_id: &str,
    challenge: &str,
    approve: bool,
    remember: bool,
) -> String {
    serde_json::to_string(&(MESSAGE_TAG, origin, vault_id, request_id, challenge, approve, remember))
        .unwrap_or_default()
}
