//! Authentificateur EC2 factice : clé P-256 jetable qui produit des preuves Hello valides pour un message.

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use sha2::{Digest, Sha256};
use vaultagent_core::encoding::{base64_encode, base64url_encode};
use vaultagent_core::{HELLO_ORIGIN, HELLO_RP_ID, HelloCredential, HelloProof, hello_challenge};

/// Authentificateur EC2 factice capable de produire des preuves Hello.
pub struct Authenticator {
    pub credential: HelloCredential,
    key: SigningKey,
}

impl Authenticator {
    /// Construit un authentificateur à partir d'un scalaire de 32 octets et d'un octet d'identifiant.
    pub fn new(scalar: &[u8], id_byte: u8) -> Self {
        let key = SigningKey::from_slice(scalar).unwrap();
        let point = key.verifying_key().to_encoded_point(false);
        let credential = HelloCredential {
            id: base64_encode(&[id_byte; 32]),
            key_type: "EC2".to_owned(),
            first: base64_encode(&point.as_bytes()[1..33]),
            second: base64_encode(&point.as_bytes()[33..65]),
        };
        Self { credential, key }
    }

    /// Preuve Hello valide pour un message.
    pub fn proof(&self, message: &str) -> HelloProof {
        let client = format!(
            r#"{{"type":"webauthn.get","challenge":"{}","origin":"{HELLO_ORIGIN}","crossOrigin":false}}"#,
            hello_challenge(message)
        );
        let mut auth = Sha256::digest(HELLO_RP_ID.as_bytes()).to_vec();
        auth.extend_from_slice(&[5, 0, 0, 0, 1]);
        let signed = [auth.clone(), Sha256::digest(client.as_bytes()).to_vec()].concat();
        let signature: Signature = self.key.sign(&signed);
        HelloProof {
            credential_id: self.credential.id.clone(),
            client_data: base64_encode(client.as_bytes()),
            authenticator_data: base64_encode(&auth),
            signature: base64_encode(signature.to_der().as_bytes()),
        }
    }
}

/// Octets d'un texte hexadécimal.
pub fn hex_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

/// Défi base64url d'un message, pour comparer avec Node.
pub fn challenge_of(message: &str) -> String {
    base64url_encode(&Sha256::digest(message.as_bytes()))
}
