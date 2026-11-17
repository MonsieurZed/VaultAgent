//! Preuve Windows Hello d'un message de décision : défi `base64url(sha256(message))`, signature par le système puis
//! vérification locale par `vaultagent-core` avant tout envoi au serveur.

use serde::Serialize;
use vaultagent_core::encoding::{base64_decode, base64_encode};
use vaultagent_core::{HELLO_ORIGIN, HELLO_RP_ID, HelloCredential, HelloProof, hello_challenge, verify_hello};

use crate::error::ClientError;
use crate::hello_system::HelloSystem;

/// Preuve envoyée au serveur : les quatre champs en base64 standard (`helloProof` du protocole).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HelloProofBody {
    /// Identifiant du justificatif utilisé.
    #[serde(rename = "credentialId")]
    pub credential_id: String,
    /// `clientData` JSON brut.
    #[serde(rename = "clientData")]
    pub client_data: String,
    /// Données de l'authentificateur.
    #[serde(rename = "authenticatorData")]
    pub authenticator_data: String,
    /// Signature ECDSA (DER) ou RSA PKCS#1 v1.5.
    pub signature: String,
}

/// Signe un message avec Windows Hello et vérifie la preuve localement.
///
/// Paramètres : le système Hello, le justificatif enrôlé et le message de décision exact.
/// Retour : la preuve, valable pour ce seul message (RP `vaultagent-client.local`, origine
/// `https://vaultagent-client.local`).
///
/// # Errors
///
/// `HelloUnavailable` si Windows Hello n'est pas disponible, `HelloCancelled` ou `Hello` si la signature échoue,
/// `Hello` si le justificatif est illisible ou si la preuve ne passe pas la vérification locale.
pub fn prove(
    system: &dyn HelloSystem,
    credential: &HelloCredential,
    message: &str,
) -> Result<HelloProofBody, ClientError> {
    if !system.status().available {
        return Err(ClientError::HelloUnavailable);
    }
    let id =
        base64_decode(&credential.id).ok_or_else(|| ClientError::Hello("The Hello key id is invalid.".to_owned()))?;
    let assertion = system.sign(&id, &hello_challenge(message), HELLO_RP_ID, HELLO_ORIGIN)?;
    let body = HelloProofBody {
        credential_id: base64_encode(&assertion.credential_id),
        client_data: base64_encode(&assertion.client_data),
        authenticator_data: base64_encode(&assertion.authenticator_data),
        signature: base64_encode(&assertion.signature),
    };
    let proof = HelloProof {
        credential_id: body.credential_id.clone(),
        client_data: body.client_data.clone(),
        authenticator_data: body.authenticator_data.clone(),
        signature: body.signature.clone(),
    };
    verify_hello(Some(credential), Some(&proof), message)
        .map_err(|_| ClientError::Hello("The Windows Hello proof did not verify locally.".to_owned()))?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_hello::FakeHello;
    use crate::hello_fixture::{credential, signer, signing_key};

    #[test]
    fn valid_proof_is_verified_and_covers_the_message() {
        let key = signing_key(9);
        let credential = credential(&key);
        let system = signer(key, 5, HELLO_ORIGIN, Err(ClientError::HelloUnavailable));
        let body = prove(&system, &credential, "message-1").unwrap();
        assert_eq!(body.credential_id, credential.id);
        let proof = HelloProof {
            credential_id: body.credential_id,
            client_data: body.client_data,
            authenticator_data: body.authenticator_data,
            signature: body.signature,
        };
        assert!(verify_hello(Some(&credential), Some(&proof), "message-1").is_ok());
        assert!(verify_hello(Some(&credential), Some(&proof), "message-2").is_err());
        assert_eq!(system.challenges(), [hello_challenge("message-1")]);
    }

    #[test]
    fn proof_serializes_with_the_protocol_field_names() {
        let key = signing_key(9);
        let credential = credential(&key);
        let system = signer(key, 5, HELLO_ORIGIN, Err(ClientError::HelloUnavailable));
        let value = serde_json::to_value(prove(&system, &credential, "m").unwrap()).unwrap();
        let mut names: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, ["authenticatorData", "clientData", "credentialId", "signature"]);
    }

    #[test]
    fn proof_failing_the_local_check_is_never_returned() {
        let credential = credential(&signing_key(9));
        let refused = Err(ClientError::HelloUnavailable);
        for system in [
            signer(signing_key(9), 1, HELLO_ORIGIN, refused.clone()),
            signer(signing_key(9), 5, "https://other.local", refused.clone()),
            signer(signing_key(3), 5, HELLO_ORIGIN, refused),
        ] {
            assert_eq!(prove(&system, &credential, "m").unwrap_err().code(), "hello-failed");
        }
    }

    #[test]
    fn unavailable_system_refuses_before_signing() {
        let credential = credential(&signing_key(9));
        let system = FakeHello::unavailable();
        assert_eq!(
            prove(&system, &credential, "m").unwrap_err().code(),
            "hello-unavailable"
        );
        assert_eq!(system.challenges().len(), 0);
    }
}
