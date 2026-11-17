//! Clé privée RSA d'approbateur en PKCS#8 DER : export et import, avec contrôle du module de 2048 bits.

use crate::error::{ErrorCode, Result};
use rsa::RsaPrivateKey;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use rsa::traits::PublicKeyParts;
use zeroize::Zeroizing;

const APPROVER_BITS: usize = 2048;

/// Exporte une clé privée en PKCS#8 DER.
///
/// Paramètres : la clé privée.
/// Retour : les octets DER, effacés à la destruction.
/// # Errors
///
/// `invalid-key` si l'encodage échoue.
pub fn export_pkcs8(key: &RsaPrivateKey) -> Result<Zeroizing<Vec<u8>>> {
    let document = key.to_pkcs8_der().map_err(|_| ErrorCode::InvalidKey)?;
    Ok(Zeroizing::new(document.as_bytes().to_vec()))
}

/// Importe une clé privée d'approbateur depuis du PKCS#8 DER.
///
/// Paramètres : les octets DER.
/// Retour : la clé, de 2048 bits exactement.
/// # Errors
///
/// `invalid-key` si les octets ne sont pas du PKCS#8 RSA valide ou si le module n'a pas 2048 bits.
pub fn import_pkcs8(der: &[u8]) -> Result<RsaPrivateKey> {
    let key = RsaPrivateKey::from_pkcs8_der(der).map_err(|_| ErrorCode::InvalidKey)?;
    if key.n().bits() != APPROVER_BITS {
        return Err(ErrorCode::InvalidKey);
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entropy::SeededEntropy;
    use crate::rsa_keys::{generate_rsa, public_key_spki};

    #[test]
    fn export_then_import_keeps_the_public_key() {
        let mut entropy = SeededEntropy::new(11);
        let key = generate_rsa(2048, &mut entropy).unwrap();
        let der = export_pkcs8(&key).unwrap();
        let back = import_pkcs8(&der).unwrap();
        assert_eq!(public_key_spki(&back).unwrap(), public_key_spki(&key).unwrap());
    }

    #[test]
    fn garbage_and_other_sizes_are_refused() {
        assert_eq!(import_pkcs8(b"not der").unwrap_err(), ErrorCode::InvalidKey);
        let mut entropy = SeededEntropy::new(12);
        let small = generate_rsa(1024, &mut entropy).unwrap();
        let der = export_pkcs8(&small).unwrap();
        assert_eq!(import_pkcs8(&der).unwrap_err(), ErrorCode::InvalidKey);
    }
}
