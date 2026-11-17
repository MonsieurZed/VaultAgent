//! Outils des tests Hello : clés de test déterministes, données d'authentificateur d'enregistrement et faux système
//! qui signe de vraies preuves WebAuthn, sans aucune invite.

#[cfg(test)]
pub(crate) use fixture::*;
#[cfg(test)]
mod fixture {
    use p256::ecdsa::signature::Signer;
    use p256::ecdsa::{Signature, SigningKey};
    use vaultagent_core::encoding::base64_encode;
    use vaultagent_core::{HELLO_ORIGIN, HELLO_RP_ID, HelloCredential, digest};

    use crate::error::ClientError;
    use crate::fake_hello::FakeHello;
    use crate::hello_system::{HelloAssertion, HelloRegistration};

    /// Identifiant du justificatif de test.
    pub const ID: [u8; 16] = [7; 16];

    /// Octets d'un hexadécimal minuscule.
    pub fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }

    /// Chaîne d'octets CBOR de 24 à 65535 octets.
    pub fn cbor_bytes(content: &[u8]) -> Vec<u8> {
        let length = u16::try_from(content.len()).unwrap().to_be_bytes();
        [&[0x59][..], &length, content].concat()
    }

    /// Clé COSE EC2 P-256 avec les coordonnées données.
    pub fn ec2_key(x: &[u8], y: &[u8]) -> Vec<u8> {
        let mut key = vec![0xa5, 0x01, 0x02, 0x03, 0x26, 0x20, 0x01, 0x21, 0x58, 0x20];
        key.extend_from_slice(x);
        key.extend_from_slice(&[0x22, 0x58, 0x20]);
        key.extend_from_slice(y);
        key
    }

    /// Clé COSE RSA avec le module donné et l'exposant 65537.
    pub fn rsa_key(modulus: &[u8]) -> Vec<u8> {
        let mut key = vec![0xa4, 0x01, 0x03, 0x03, 0x39, 0x01, 0x00, 0x20];
        key.extend_from_slice(&cbor_bytes(modulus));
        key.extend_from_slice(&[0x21, 0x43, 0x01, 0x00, 0x01]);
        key
    }

    /// Données d'authentificateur d'attestation avec le site, les drapeaux et la clé COSE donnés.
    pub fn registration(rp: &str, flags: u8, cose: &[u8]) -> HelloRegistration {
        let mut auth = unhex(&digest(rp.as_bytes()));
        auth.push(flags);
        auth.extend_from_slice(&[0, 0, 0, 1]);
        auth.extend_from_slice(&[0; 16]);
        auth.extend_from_slice(&u16::try_from(ID.len()).unwrap().to_be_bytes());
        auth.extend_from_slice(&ID);
        auth.extend_from_slice(cose);
        HelloRegistration {
            credential_id: ID.to_vec(),
            authenticator_data: auth,
        }
    }

    /// Point P-256 valide : le générateur.
    pub fn generator() -> (Vec<u8>, Vec<u8>) {
        (
            unhex("6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"),
            unhex("4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"),
        )
    }

    /// Clé de signature déterministe.
    pub fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_slice(&[seed; 32]).unwrap()
    }

    /// Justificatif EC2 de la clé donnée, avec l'identifiant de test.
    pub fn credential(key: &SigningKey) -> HelloCredential {
        let point = key.verifying_key().to_encoded_point(false);
        HelloCredential {
            id: base64_encode(&ID),
            key_type: "EC2".to_owned(),
            first: base64_encode(point.x().unwrap()),
            second: base64_encode(point.y().unwrap()),
        }
    }

    /// Enregistrement valide de la clé donnée pour le site de production.
    pub fn key_registration(key: &SigningKey) -> HelloRegistration {
        let point = key.verifying_key().to_encoded_point(false);
        registration(HELLO_RP_ID, 0x45, &ec2_key(point.x().unwrap(), point.y().unwrap()))
    }

    /// Système dont la signature fabrique une preuve pour le défi reçu : drapeaux et origine au choix.
    ///
    /// Paramètres : la clé qui signe, les drapeaux, l'origine du `clientData` et le résultat de l'enregistrement.
    pub fn signer(
        key: SigningKey,
        flags: u8,
        origin: &'static str,
        registered: Result<HelloRegistration, ClientError>,
    ) -> FakeHello {
        let script = move |id: &[u8], challenge: &str| {
            let client = format!(
                r#"{{"type":"webauthn.get","challenge":"{challenge}","origin":"{origin}","crossOrigin":false}}"#
            )
            .into_bytes();
            let mut auth = unhex(&digest(HELLO_RP_ID.as_bytes()));
            auth.extend_from_slice(&[flags, 0, 0, 0, 1]);
            let signed = [auth.clone(), unhex(&digest(&client))].concat();
            let signature: Signature = key.sign(&signed);
            Ok(HelloAssertion {
                credential_id: id.to_vec(),
                client_data: client,
                authenticator_data: auth,
                signature: signature.to_der().as_bytes().to_vec(),
            })
        };
        FakeHello::new(registered, Box::new(script))
    }

    /// Système qui signe correctement pour la clé donnée et crée cette même clé.
    pub fn working_hello(key: &SigningKey) -> FakeHello {
        signer(key.clone(), 5, HELLO_ORIGIN, Ok(key_registration(key)))
    }
}
