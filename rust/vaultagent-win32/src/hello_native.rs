//! Appels FFI bruts de `webauthn.dll` (Windows Hello) : disponibilité, création d'un justificatif, assertion. Chaque
//! structure rendue par Windows est copiée puis libérée par la fonction de libération qui lui correspond.

use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Networking::WindowsWebServices::{
    WEBAUTHN_ASSERTION, WEBAUTHN_AUTHENTICATOR_ATTACHMENT_PLATFORM, WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS,
    WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS, WEBAUTHN_CLIENT_DATA, WEBAUTHN_COSE_CREDENTIAL_PARAMETER,
    WEBAUTHN_COSE_CREDENTIAL_PARAMETERS, WEBAUTHN_CREDENTIAL, WEBAUTHN_CREDENTIAL_ATTESTATION,
    WEBAUTHN_CREDENTIAL_TYPE_PUBLIC_KEY, WEBAUTHN_HASH_ALGORITHM_SHA_256, WEBAUTHN_RP_ENTITY_INFORMATION,
    WEBAUTHN_USER_ENTITY_INFORMATION, WEBAUTHN_USER_VERIFICATION_REQUIREMENT_REQUIRED,
    WebAuthNAuthenticatorGetAssertion, WebAuthNAuthenticatorMakeCredential, WebAuthNFreeAssertion,
    WebAuthNFreeCredentialAttestation, WebAuthNGetApiVersionNumber,
    WebAuthNIsUserVerifyingPlatformAuthenticatorAvailable,
};

use crate::error::Win32Error;
use crate::hello::{Assertion, Registration};
use crate::native::hwnd;

const API_VERSION_1: u32 = 1;
const TIMEOUT_MILLISECONDS: u32 = 60_000;
const ATTESTATION_NONE: u32 = 1;
const ES256: i32 = -7;
const RS256: i32 = -257;
const MAX_RESULT_BYTES: u32 = 1024 * 1024;
const USER_CANCELLED: u32 = 0x8009_0036;
const WIN32_CANCELLED: u32 = 0x8007_04C7;
const WIN32_TIMEOUT: u32 = 0x8007_05B4;

/// Convertit un texte en UTF-16 terminé par un zéro.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Traduit un `HRESULT` de `webauthn.dll`.
///
/// Paramètres : le code rendu et le nom de la fonction appelée.
/// Retour : `Ok` pour zéro, `Cancelled` ou `TimedOut` pour l'annulation et le délai de l'invite, `Api` sinon.
fn check(result: i32, call: &'static str) -> Result<(), Win32Error> {
    let code = result.cast_unsigned();
    match code {
        0 => Ok(()),
        USER_CANCELLED | WIN32_CANCELLED => Err(Win32Error::Cancelled),
        WIN32_TIMEOUT => Err(Win32Error::TimedOut),
        _ => Err(Win32Error::Api { call, code }),
    }
}

/// Copie un tampon rendu par Windows.
///
/// Paramètres : l'adresse et la longueur lues dans une structure rendue par `webauthn.dll`.
/// Retour : la copie, ou une liste vide si l'adresse est nulle ou la longueur nulle ou supérieure à 1 Mio.
///
/// Sûreté : l'appelant garantit que l'adresse désigne `length` octets valides tant que la structure n'est pas
/// libérée ; la copie est faite avant la libération.
unsafe fn copy_bytes(address: *const u8, length: u32) -> Vec<u8> {
    if address.is_null() || length == 0 || length > MAX_RESULT_BYTES {
        return Vec::new();
    }
    let count = usize::try_from(length).unwrap_or(0);
    unsafe { std::slice::from_raw_parts(address, count) }.to_vec()
}

/// Interroge la disponibilité d'un authentificateur de plateforme avec vérification de l'utilisateur.
///
/// Retour : `true` si Windows Hello est utilisable ; aucune invite n'est affichée.
///
/// Sûreté : le paramètre de sortie est un booléen local valide pour toute la durée de l'appel.
pub(crate) fn platform_available() -> bool {
    let mut available = 0;
    let result = unsafe { WebAuthNIsUserVerifyingPlatformAuthenticatorAvailable(&raw mut available) };
    result == 0 && available != 0
}

/// Lit le numéro de version de l'API WebAuthn de Windows.
///
/// Retour : la version, sans invite.
///
/// Sûreté : la fonction n'a ni paramètre ni précondition.
pub(crate) fn api_version() -> u32 {
    unsafe { WebAuthNGetApiVersionNumber() }
}

/// Appelle `WebAuthNAuthenticatorMakeCredential`.
///
/// Paramètres : la fenêtre parente, puis les structures d'entrée, vivantes pendant l'appel.
/// Retour : le `HRESULT` et le pointeur de résultat (nul en cas d'échec).
///
/// Sûreté : chaque pointeur désigne une structure initialisée dont les tampons internes restent vivants jusqu'au
/// retour ; le pointeur de sortie est une variable locale.
unsafe fn call_make(
    window: HWND,
    entities: (&WEBAUTHN_RP_ENTITY_INFORMATION, &WEBAUTHN_USER_ENTITY_INFORMATION),
    algorithms: &WEBAUTHN_COSE_CREDENTIAL_PARAMETERS,
    client: &WEBAUTHN_CLIENT_DATA,
    options: &WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS,
) -> (i32, *mut WEBAUTHN_CREDENTIAL_ATTESTATION) {
    let mut result = null_mut();
    let code = unsafe {
        WebAuthNAuthenticatorMakeCredential(
            window,
            entities.0,
            entities.1,
            algorithms,
            client,
            options,
            &raw mut result,
        )
    };
    (code, result)
}

/// Copie l'attestation puis la libère.
///
/// Paramètres : le pointeur rendu par un appel réussi.
/// Retour : l'identifiant du justificatif et les données de l'authentificateur.
///
/// Sûreté : le pointeur vient d'un `WebAuthNAuthenticatorMakeCredential` réussi, il est lu avant d'être libéré une
/// seule fois par `WebAuthNFreeCredentialAttestation`.
unsafe fn take_attestation(result: *mut WEBAUTHN_CREDENTIAL_ATTESTATION) -> Registration {
    let value = unsafe { &*result };
    let registration = Registration {
        credential_id: unsafe { copy_bytes(value.pbCredentialId, value.cbCredentialId) },
        authenticator_data: unsafe { copy_bytes(value.pbAuthenticatorData, value.cbAuthenticatorData) },
    };
    unsafe { WebAuthNFreeCredentialAttestation(result) };
    registration
}

/// Crée un justificatif Windows Hello (invite Hello visible).
///
/// Paramètres : la fenêtre parente, le site (identifiant et nom), l'utilisateur (identifiant et nom) et le
/// `clientData` JSON.
/// Retour : l'identifiant et les données de l'authentificateur ; `Cancelled`, `TimedOut` ou `Api` sinon, et `Api`
/// aussi si Windows rend un résultat vide.
///
/// Sûreté : les pointeurs passés aux appels `unsafe` désignent des tampons locaux vivants jusqu'au retour, et le
/// résultat n'est lu et libéré qu'après un code de succès.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub(crate) fn make_credential(
    window: isize,
    site: (&str, &str),
    user: (&[u8], &str),
    client_json: &[u8],
) -> Result<Registration, Win32Error> {
    let (rp_id, rp_name, user_name) = (wide(site.0), wide(site.1), wide(user.1));
    let rp = WEBAUTHN_RP_ENTITY_INFORMATION {
        dwVersion: API_VERSION_1,
        pwszId: rp_id.as_ptr(),
        pwszName: rp_name.as_ptr(),
        pwszIcon: null(),
    };
    let user_id = user.0.to_vec();
    let entity = WEBAUTHN_USER_ENTITY_INFORMATION {
        dwVersion: API_VERSION_1,
        cbId: u32::try_from(user_id.len()).unwrap_or(0),
        pbId: user_id.as_ptr().cast_mut(),
        pwszName: user_name.as_ptr(),
        pwszIcon: null(),
        pwszDisplayName: user_name.as_ptr(),
    };
    let mut parameters = [ES256, RS256].map(|algorithm| WEBAUTHN_COSE_CREDENTIAL_PARAMETER {
        dwVersion: API_VERSION_1,
        pwszCredentialType: WEBAUTHN_CREDENTIAL_TYPE_PUBLIC_KEY,
        lAlg: algorithm,
    });
    let algorithms = WEBAUTHN_COSE_CREDENTIAL_PARAMETERS {
        cCredentialParameters: 2,
        pCredentialParameters: parameters.as_mut_ptr(),
    };
    let client = client_data(client_json);
    let options = WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS {
        dwVersion: API_VERSION_1,
        dwTimeoutMilliseconds: TIMEOUT_MILLISECONDS,
        dwAuthenticatorAttachment: WEBAUTHN_AUTHENTICATOR_ATTACHMENT_PLATFORM,
        dwUserVerificationRequirement: WEBAUTHN_USER_VERIFICATION_REQUIREMENT_REQUIRED,
        dwAttestationConveyancePreference: ATTESTATION_NONE,
        ..Default::default()
    };
    let (code, result) = unsafe { call_make(hwnd(window), (&rp, &entity), &algorithms, &client, &options) };
    check(code, "WebAuthNAuthenticatorMakeCredential")?;
    if result.is_null() {
        return Err(Win32Error::Api {
            call: "WebAuthNAuthenticatorMakeCredential",
            code: 0,
        });
    }
    Ok(unsafe { take_attestation(result) })
}

/// Décrit le `clientData` JSON pour Windows, haché en SHA-256.
///
/// Paramètres : les octets JSON, vivants pendant l'appel qui reçoit la structure.
/// Retour : la structure `WEBAUTHN_CLIENT_DATA`.
fn client_data(json: &[u8]) -> WEBAUTHN_CLIENT_DATA {
    WEBAUTHN_CLIENT_DATA {
        dwVersion: API_VERSION_1,
        cbClientDataJSON: u32::try_from(json.len()).unwrap_or(0),
        pbClientDataJSON: json.as_ptr().cast_mut(),
        pwszHashAlgId: WEBAUTHN_HASH_ALGORITHM_SHA_256,
    }
}

/// Appelle `WebAuthNAuthenticatorGetAssertion`.
///
/// Paramètres : la fenêtre parente, le site en UTF-16, le `clientData` et les options, vivants pendant l'appel.
/// Retour : le `HRESULT` et le pointeur de résultat (nul en cas d'échec).
///
/// Sûreté : chaque pointeur désigne une structure initialisée dont les tampons internes restent vivants jusqu'au
/// retour ; le pointeur de sortie est une variable locale.
unsafe fn call_get(
    window: HWND,
    rp_id: &[u16],
    client: &WEBAUTHN_CLIENT_DATA,
    options: &WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS,
) -> (i32, *mut WEBAUTHN_ASSERTION) {
    let mut result = null_mut();
    let code = unsafe { WebAuthNAuthenticatorGetAssertion(window, rp_id.as_ptr(), client, options, &raw mut result) };
    (code, result)
}

/// Copie l'assertion puis la libère.
///
/// Paramètres : le pointeur rendu par un appel réussi et le `clientData` envoyé.
/// Retour : l'assertion avec le `clientData` utilisé.
///
/// Sûreté : le pointeur vient d'un `WebAuthNAuthenticatorGetAssertion` réussi, il est lu avant d'être libéré une
/// seule fois par `WebAuthNFreeAssertion`.
unsafe fn take_assertion(result: *mut WEBAUTHN_ASSERTION, client_json: &[u8]) -> Assertion {
    let value = unsafe { &*result };
    let assertion = Assertion {
        credential_id: unsafe { copy_bytes(value.Credential.pbId, value.Credential.cbId) },
        client_data: client_json.to_vec(),
        authenticator_data: unsafe { copy_bytes(value.pbAuthenticatorData, value.cbAuthenticatorData) },
        signature: unsafe { copy_bytes(value.pbSignature, value.cbSignature) },
    };
    unsafe { WebAuthNFreeAssertion(result) };
    assertion
}

/// Signe un défi avec un justificatif existant (invite Hello visible).
///
/// Paramètres : la fenêtre parente, l'identifiant du site, l'identifiant du justificatif et le `clientData` JSON.
/// Retour : l'assertion ; `Cancelled`, `TimedOut` ou `Api` sinon, et `Api` aussi si Windows rend un résultat vide.
///
/// Sûreté : les pointeurs passés aux appels `unsafe` désignent des tampons locaux vivants jusqu'au retour, et le
/// résultat n'est lu et libéré qu'après un code de succès.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub(crate) fn get_assertion(
    window: isize,
    rp_id: &str,
    credential_id: &[u8],
    client_json: &[u8],
) -> Result<Assertion, Win32Error> {
    let rp = wide(rp_id);
    let mut credential = WEBAUTHN_CREDENTIAL {
        dwVersion: API_VERSION_1,
        cbId: u32::try_from(credential_id.len()).unwrap_or(0),
        pbId: credential_id.as_ptr().cast_mut(),
        pwszCredentialType: WEBAUTHN_CREDENTIAL_TYPE_PUBLIC_KEY,
    };
    let mut options = WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS {
        dwVersion: API_VERSION_1,
        dwTimeoutMilliseconds: TIMEOUT_MILLISECONDS,
        dwAuthenticatorAttachment: WEBAUTHN_AUTHENTICATOR_ATTACHMENT_PLATFORM,
        dwUserVerificationRequirement: WEBAUTHN_USER_VERIFICATION_REQUIREMENT_REQUIRED,
        ..Default::default()
    };
    options.CredentialList.cCredentials = 1;
    options.CredentialList.pCredentials = &raw mut credential;
    let client = client_data(client_json);
    let (code, result) = unsafe { call_get(hwnd(window), &rp, &client, &options) };
    check(code, "WebAuthNAuthenticatorGetAssertion")?;
    if result.is_null() {
        return Err(Win32Error::Api {
            call: "WebAuthNAuthenticatorGetAssertion",
            code: 0,
        });
    }
    Ok(unsafe { take_assertion(result, client_json) })
}
