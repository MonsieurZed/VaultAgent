//! Windows Hello par WebAuthn (`webauthn.dll`) : disponibilité, enregistrement d'un justificatif de plateforme et
//! signature d'un défi. Le PIN et la biométrie restent dans l'interface de Windows ; rien n'est affiché par
//! `hello_available`.

use windows_sys::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};

use crate::error::Win32Error;
use crate::hello_native::{api_version, get_assertion, make_credential, platform_available};
use crate::native::foreground_handle;

const CHALLENGE_BYTES: usize = 32;
const RP_DISPLAY_NAME: &str = "Coffre Bitwarden";
const BASE64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Disponibilité de Windows Hello sur ce poste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelloAvailability {
    /// Un authentificateur de plateforme avec vérification de l'utilisateur est utilisable.
    pub available: bool,
    /// Version de l'API WebAuthn de Windows (0 si `webauthn.dll` est absente).
    pub api_version: u32,
}

/// Justificatif créé : identifiant et données de l'authentificateur, qui contiennent la clé publique COSE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    /// Identifiant du justificatif.
    pub credential_id: Vec<u8>,
    /// Données de l'authentificateur de l'attestation (hachage du RP, drapeaux, compteur, clé COSE).
    pub authenticator_data: Vec<u8>,
}

/// Preuve de signature : les quatre éléments d'une preuve WebAuthn `get`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assertion {
    /// Identifiant du justificatif utilisé.
    pub credential_id: Vec<u8>,
    /// `clientData` JSON envoyé à l'authentificateur.
    pub client_data: Vec<u8>,
    /// Données de l'authentificateur signées.
    pub authenticator_data: Vec<u8>,
    /// Signature ECDSA (DER) ou RSA PKCS#1 v1.5.
    pub signature: Vec<u8>,
}

/// Interroge Windows Hello sans afficher d'invite.
///
/// Retour : la disponibilité et la version de l'API ; `available` est faux si `webauthn.dll` refuse la question.
#[must_use]
pub fn hello_available() -> HelloAvailability {
    HelloAvailability {
        available: platform_available(),
        api_version: api_version(),
    }
}

/// Fenêtre parente des invites : la fenêtre au premier plan.
///
/// Retour : son identifiant, ou 0 si aucune fenêtre n'est active (Windows choisit alors le bureau).
#[must_use]
pub fn hello_window() -> isize {
    foreground_handle()
}

/// Encode en `base64url` sans remplissage.
///
/// Paramètres : les octets.
/// Retour : le texte.
fn base64url(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut group = [0u8; 3];
        group[..chunk.len()].copy_from_slice(chunk);
        let value = u32::from(group[0]) << 16 | u32::from(group[1]) << 8 | u32::from(group[2]);
        for index in 0..=chunk.len() {
            let sextet = usize::try_from(value >> (18 - 6 * index) & 63).unwrap_or(0);
            text.push(char::from(BASE64URL[sextet]));
        }
    }
    text
}

/// Tire des octets aléatoires du système.
///
/// Paramètres : le nombre d'octets.
/// Retour : les octets, ou `Api` si le générateur échoue.
///
/// Sûreté : le tampon est une liste locale de la taille annoncée ; l'indicateur `USE_SYSTEM_PREFERRED_RNG` autorise
/// un algorithme nul.
fn random_bytes(count: usize) -> Result<Vec<u8>, Win32Error> {
    let mut bytes = vec![0u8; count];
    let length = u32::try_from(count).unwrap_or(0);
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            length,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status == 0 {
        Ok(bytes)
    } else {
        Err(Win32Error::Api {
            call: "BCryptGenRandom",
            code: status.cast_unsigned(),
        })
    }
}

/// Compose le `clientData` JSON compact d'une opération WebAuthn.
///
/// Paramètres : le type (`webauthn.create` ou `webauthn.get`), le défi `base64url` et l'origine.
/// Retour : les octets JSON ; `InvalidArgument` si le défi ou l'origine contient un caractère qui exigerait un
/// échappement.
fn client_json(kind: &str, challenge: &str, origin: &str) -> Result<Vec<u8>, Win32Error> {
    let plain = |text: &str| {
        !text.is_empty()
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/'))
    };
    if !plain(challenge) || !plain(origin) {
        return Err(Win32Error::InvalidArgument);
    }
    Ok(
        format!(r#"{{"type":"{kind}","challenge":"{challenge}","origin":"{origin}","crossOrigin":false}}"#)
            .into_bytes(),
    )
}

/// Crée un justificatif Windows Hello (l'invite Hello apparaît).
///
/// Paramètres : la fenêtre parente, l'identifiant et le nom de l'utilisateur, l'identifiant du site (RP) et
/// l'origine du `clientData`.
/// Retour : l'identifiant du justificatif et les données de l'authentificateur, dont la clé publique COSE.
///
/// Sûreté : les appels `unsafe` sont dans `hello_native`, où chaque tampon reste vivant pendant l'appel.
///
/// # Errors
///
/// `Cancelled` si l'utilisateur annule, `TimedOut` après 60 s, `InvalidArgument` si le RP ou l'origine est refusé,
/// `Api` pour tout autre refus de Windows.
pub fn hello_register(
    window: isize,
    user_id: &[u8],
    user_name: &str,
    rp: &str,
    origin: &str,
) -> Result<Registration, Win32Error> {
    let challenge = base64url(&random_bytes(CHALLENGE_BYTES)?);
    let json = client_json("webauthn.create", &challenge, origin)?;
    make_credential(window, (rp, RP_DISPLAY_NAME), (user_id, user_name), &json)
}

/// Signe un défi avec un justificatif existant (l'invite Hello apparaît).
///
/// Paramètres : la fenêtre parente, l'identifiant du justificatif, le défi `base64url(sha256(message))`, le RP et
/// l'origine du `clientData`.
/// Retour : l'assertion (identifiant, `clientData`, données de l'authentificateur, signature).
///
/// Sûreté : les appels `unsafe` sont dans `hello_native`, où chaque tampon reste vivant pendant l'appel.
///
/// # Errors
///
/// `Cancelled` si l'utilisateur annule, `TimedOut` après 60 s, `InvalidArgument` si le défi, le RP ou l'origine est
/// refusé, `Api` pour tout autre refus de Windows.
pub fn hello_sign(
    window: isize,
    credential_id: &[u8],
    challenge_digest: &str,
    rp: &str,
    origin: &str,
) -> Result<Assertion, Win32Error> {
    let json = client_json("webauthn.get", challenge_digest, origin)?;
    get_assertion(window, rp, credential_id, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_matches_the_known_vectors() {
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
    }

    #[test]
    fn client_json_is_compact_and_refuses_escapes() {
        let json = client_json("webauthn.get", "abc_-", "https://x.local").unwrap();
        assert_eq!(
            String::from_utf8(json).unwrap(),
            r#"{"type":"webauthn.get","challenge":"abc_-","origin":"https://x.local","crossOrigin":false}"#
        );
        assert_eq!(
            client_json("webauthn.get", "a\"b", "https://x.local"),
            Err(Win32Error::InvalidArgument)
        );
        assert_eq!(client_json("webauthn.get", "abc", ""), Err(Win32Error::InvalidArgument));
    }

    #[test]
    fn random_bytes_have_the_requested_length_and_differ() {
        let (first, second) = (random_bytes(32).unwrap(), random_bytes(32).unwrap());
        assert_eq!(first.len(), 32);
        assert_ne!(first, second);
    }

    #[test]
    fn availability_query_shows_no_prompt_and_reports_a_version() {
        let availability = hello_available();
        println!("hello disponible : {availability:?}");
        assert!(availability.api_version > 0);
    }
}
