//! Clés RSA côté client : génération, export SPKI et signature RSA-PSS SHA-256 de sel 32 octets, pour les PC
//! approbateurs et les outils de contrôle (le serveur, lui, ne signe jamais).

use crate::approver_key::{decision_message, verify_pss};
use crate::encoding::base64_encode;
use crate::entropy::{Entropy, EntropyRng};
use crate::error::{ErrorCode, Result};
use rsa::RsaPrivateKey;
use rsa::pkcs8::EncodePublicKey;
use rsa::pss::SigningKey;
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use sha2::Sha256;

const SALT_LEN: usize = 32;

/// Code d'erreur rendu quand une opération de clé échoue.
fn key_failure() -> ErrorCode {
    ErrorCode::InvalidKey
}

/// Génère une clé privée RSA.
///
/// Paramètres : la taille du module en bits, la source d'aléa.
/// Retour : la clé privée.
/// # Errors
///
/// `invalid-key` si la génération échoue ; `engine-entropy` si l'aléa est indisponible.
pub fn generate_rsa(bits: usize, entropy: &mut impl Entropy) -> Result<RsaPrivateKey> {
    let mut rng = EntropyRng::new(entropy);
    let key = RsaPrivateKey::new(&mut rng, bits);
    rng.finish()?;
    key.map_err(|_| key_failure())
}

/// Exporte la clé publique d'une clé privée en SPKI DER encodé en base64.
///
/// Paramètres : la clé privée.
/// Retour : le texte base64.
/// # Errors
///
/// `invalid-key` si l'encodage échoue.
pub fn public_key_spki(key: &RsaPrivateKey) -> Result<String> {
    let der = key.to_public_key().to_public_key_der().map_err(|_| key_failure())?;
    Ok(base64_encode(der.as_bytes()))
}

/// Signe un message en RSA-PSS SHA-256 de sel 32 octets.
///
/// Paramètres : la clé privée, le message, la source d'aléa.
/// Retour : la signature en base64.
/// # Errors
///
/// `engine-entropy` si l'aléa est indisponible.
pub fn sign_pss(key: &RsaPrivateKey, message: &str, entropy: &mut impl Entropy) -> Result<String> {
    let mut rng = EntropyRng::new(entropy);
    let signature =
        SigningKey::<Sha256>::new_with_salt_len(key.clone(), SALT_LEN).sign_with_rng(&mut rng, message.as_bytes());
    rng.finish()?;
    Ok(base64_encode(&signature.to_bytes()))
}

/// Signe une décision d'approbateur `VaultAgent.Unlock.v2`.
///
/// Paramètres : la clé privée, l'origine, le coffre, la demande, le défi, la décision, le souhait de mémoriser, la
/// source d'aléa.
/// Retour : la signature en base64.
/// # Errors
///
/// `engine-entropy` si l'aléa est indisponible.
pub fn sign_decision(
    key: &RsaPrivateKey,
    context: (&str, &str, &str, &str),
    approve: bool,
    remember: bool,
    entropy: &mut impl Entropy,
) -> Result<String> {
    let (origin, vault_id, request_id, challenge) = context;
    let message = decision_message(origin, vault_id, request_id, challenge, approve, remember);
    sign_pss(key, &message, entropy)
}

/// Vérifie qu'une signature produite par [`sign_decision`] est acceptée par la clé publique correspondante.
///
/// Paramètres : la clé privée, le message de la décision.
/// Retour : rien.
/// # Errors
///
/// `invalid-key` si la signature n'est pas valide.
pub fn check_signature(key: &RsaPrivateKey, message: &str, signature: &str) -> Result<()> {
    verify_pss(&public_key_spki(key)?, message, signature, key_failure())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entropy::SeededEntropy;

    #[test]
    fn generated_key_signs_and_verifies() {
        let mut entropy = SeededEntropy::new(7);
        let key = generate_rsa(2048, &mut entropy).unwrap();
        let context = ("https://localhost", "default", "req", "challenge");
        let signature = sign_decision(&key, context, true, false, &mut entropy).unwrap();
        let message = decision_message("https://localhost", "default", "req", "challenge", true, false);
        check_signature(&key, &message, &signature).unwrap();
        assert!(check_signature(&key, "autre", &signature).is_err());
    }

    #[test]
    fn exported_key_is_accepted_as_approver_key() {
        let mut entropy = SeededEntropy::new(8);
        let key = generate_rsa(2048, &mut entropy).unwrap();
        assert!(crate::approver_key::rsa_public_key(&public_key_spki(&key).unwrap()).is_ok());
    }
}
