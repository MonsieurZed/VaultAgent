//! Vérification des preuves WebAuthn de Windows Hello (`hello.mjs`) : seule la clé publique est conservée côté
//! serveur, la preuve doit couvrir le message de décision exact.

use crate::device_record::HelloCredential;
use crate::encoding::{base64_decode, base64_encode, base64url_encode};
use crate::error::{ErrorCode, Result};
use p256::EncodedPoint;
use p256::ecdsa::{Signature as EcSignature, VerifyingKey as EcKey};
use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey as RsaKey};
use rsa::signature::Verifier;
use rsa::{BigUint, RsaPublicKey};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Identifiant du site de confiance WebAuthn.
pub const HELLO_RP_ID: &str = "vaultagent-client.local";
/// Origine attendue dans `clientData`.
pub const HELLO_ORIGIN: &str = "https://vaultagent-client.local";
const MAX_ID: usize = 4096;
const MAX_COMPONENT: usize = 512;
const MAX_CLIENT_DATA: usize = 16_384;
const MAX_AUTHENTICATOR: usize = 4096;
const MAX_SIGNATURE: usize = 1024;
const MIN_ID: usize = 16;
const COORDINATE_LEN: usize = 32;
const MIN_RSA_MODULUS: usize = 256;
const MAX_RSA_EXPONENT: usize = 4;
const FLAGS_OFFSET: usize = 32;
const FLAGS_PRESENT_AND_VERIFIED: u8 = 5;
const MIN_AUTHENTICATOR_LEN: usize = 37;

/// Preuve renvoyée par l'authentificateur : quatre champs en base64 standard. Un champ absent reste vide et sera
/// refusé comme `hello-invalid`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HelloProof {
    /// Identifiant du justificatif utilisé.
    pub credential_id: String,
    /// `clientData` JSON brut.
    pub client_data: String,
    /// Données de l'authentificateur (hachage du RP, drapeaux, compteur).
    pub authenticator_data: String,
    /// Signature ECDSA (DER) ou RSA PKCS#1 v1.5.
    pub signature: String,
}

/// Clé publique reconstruite à partir d'un justificatif enrôlé.
pub enum HelloKey {
    /// Courbe P-256 (type `EC2`).
    Ec(EcKey),
    /// RSA (type `RSA`).
    Rsa(RsaPublicKey),
}

/// Défi attendu pour un message : `base64url(sha256(message))`.
///
/// Paramètres : le message de décision.
/// Retour : le défi sans remplissage.
#[must_use]
pub fn hello_challenge(message: &str) -> String {
    base64url_encode(&Sha256::digest(message.as_bytes()))
}

/// Décode du base64 standard strict : remplissage canonique, non vide, taille bornée.
fn decode(value: &str, maximum: usize) -> Result<Vec<u8>> {
    if value.len() > maximum * 2 {
        return Err(ErrorCode::HelloInvalid);
    }
    base64_decode(value)
        .filter(|bytes| !bytes.is_empty() && bytes.len() <= maximum && base64_encode(bytes) == value)
        .ok_or(ErrorCode::HelloInvalid)
}

/// Construit la clé P-256 à partir des coordonnées x et y.
fn ec_key(x: &[u8], y: &[u8]) -> Result<EcKey> {
    let point = [&[4u8][..], x, y].concat();
    let encoded = EncodedPoint::from_bytes(&point).map_err(|_| ErrorCode::HelloInvalid)?;
    EcKey::from_encoded_point(&encoded).map_err(|_| ErrorCode::HelloInvalid)
}

/// Reconstruit la clé publique d'un justificatif Hello.
///
/// Paramètres : le justificatif enrôlé, absent compté comme invalide.
/// Retour : la clé EC2 (x et y de 32 octets) ou RSA (module d'au moins 256 octets, exposant d'au plus 4 octets).
/// # Errors
///
/// `hello-invalid` pour un justificatif absent, mal encodé, de type inconnu ou de taille refusée.
pub fn hello_public_key(credential: Option<&HelloCredential>) -> Result<HelloKey> {
    let credential = credential.ok_or(ErrorCode::HelloInvalid)?;
    if decode(&credential.id, MAX_ID)?.len() < MIN_ID {
        return Err(ErrorCode::HelloInvalid);
    }
    let first = decode(&credential.first, MAX_COMPONENT)?;
    let second = decode(&credential.second, MAX_COMPONENT)?;
    match credential.key_type.as_str() {
        "EC2" if first.len() == COORDINATE_LEN && second.len() == COORDINATE_LEN => {
            ec_key(&first, &second).map(HelloKey::Ec)
        }
        "RSA" if first.len() >= MIN_RSA_MODULUS && second.len() <= MAX_RSA_EXPONENT => {
            RsaPublicKey::new(BigUint::from_bytes_be(&first), BigUint::from_bytes_be(&second))
                .map(HelloKey::Rsa)
                .map_err(|_| ErrorCode::HelloInvalid)
        }
        _ => Err(ErrorCode::HelloInvalid),
    }
}

/// Contrôle le contenu de `clientData` : type, origine, défi et absence de contexte croisé.
fn check_client_data(client_data: &[u8], message: &str) -> Result<()> {
    let client: Value = std::str::from_utf8(client_data)
        .ok()
        .and_then(|text| serde_json::from_str(text).ok())
        .ok_or(ErrorCode::HelloInvalid)?;
    let text = |key: &str| client.get(key).and_then(Value::as_str);
    let accepted = text("type") == Some("webauthn.get")
        && text("origin") == Some(HELLO_ORIGIN)
        && text("challenge") == Some(hello_challenge(message).as_str())
        && client.get("crossOrigin") == Some(&Value::Bool(false));
    accepted.then_some(()).ok_or(ErrorCode::HelloInvalid)
}

/// Contrôle les données de l'authentificateur : hachage du RP et drapeaux présence et vérification utilisateur.
fn check_authenticator(auth: &[u8]) -> Result<()> {
    let rp_hash = Sha256::digest(HELLO_RP_ID.as_bytes());
    let accepted = auth.len() >= MIN_AUTHENTICATOR_LEN
        && bool::from(auth[..FLAGS_OFFSET].ct_eq(rp_hash.as_slice()))
        && auth[FLAGS_OFFSET] & FLAGS_PRESENT_AND_VERIFIED == FLAGS_PRESENT_AND_VERIFIED;
    accepted.then_some(()).ok_or(ErrorCode::HelloInvalid)
}

/// Vérifie la signature de `authenticatorData || sha256(clientData)` avec la clé du justificatif.
fn check_signature(key: &HelloKey, signed: &[u8], signature: &[u8]) -> Result<()> {
    let verified = match key {
        HelloKey::Ec(key) => EcSignature::from_der(signature).is_ok_and(|value| key.verify(signed, &value).is_ok()),
        HelloKey::Rsa(key) => RsaSignature::try_from(signature)
            .is_ok_and(|value| RsaKey::<Sha256>::new(key.clone()).verify(signed, &value).is_ok()),
    };
    verified.then_some(()).ok_or(ErrorCode::HelloInvalid)
}

/// Vérifie qu'une preuve Hello couvre exactement un message de décision.
///
/// Paramètres : le justificatif enrôlé, la preuve reçue (absente : exigée), le message signé.
/// Retour : rien si tout concorde (identifiant, `clientData`, authentificateur, signature).
/// # Errors
///
/// `hello-invalid` pour toute incohérence ; `hello-required` si la preuve est absente alors que la clé est valide.
pub fn verify_hello(credential: Option<&HelloCredential>, proof: Option<&HelloProof>, message: &str) -> Result<()> {
    let key = hello_public_key(credential)?;
    let (credential, proof) = credential.zip(proof).ok_or(ErrorCode::HelloRequired)?;
    let id = decode(&proof.credential_id, MAX_ID)?;
    let expected = decode(&credential.id, MAX_ID)?;
    if id.len() != expected.len() || !bool::from(id.ct_eq(&expected)) {
        return Err(ErrorCode::HelloInvalid);
    }
    let client_data = decode(&proof.client_data, MAX_CLIENT_DATA)?;
    check_client_data(&client_data, message)?;
    let auth = decode(&proof.authenticator_data, MAX_AUTHENTICATOR)?;
    check_authenticator(&auth)?;
    let signed = [auth.as_slice(), Sha256::digest(&client_data).as_slice()].concat();
    check_signature(&key, &signed, &decode(&proof.signature, MAX_SIGNATURE)?)
}
