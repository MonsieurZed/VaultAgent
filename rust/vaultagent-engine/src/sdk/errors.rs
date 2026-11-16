//! Classement des erreurs du SDK Bitwarden en codes `<étape>:<catégorie>` : par type d'erreur, jamais par texte
//! de message hors la recherche d'un échec TLS dans la chaîne d'une erreur réseau. Aucun message du SDK ou du
//! serveur n'entre dans le code rendu : un secret ne peut pas s'y glisser.

use std::error::Error;

use bitwarden_api_api::ApiError;
use bitwarden_core::auth::login::{LoginError, PreloginError};
use bitwarden_sync::SyncError;
use vaultagent_core::ErrorCode;

use crate::engine::stage_error;

/// Catégorie d'un statut HTTP de réponse d'API.
///
/// Paramètres : le statut numérique.
/// Retour : `credentials` pour 401 et 403, `network` pour 408, 429 et les 5xx, sinon `rejected`.
#[must_use]
pub fn status_category(status: u16) -> &'static str {
    match status {
        401 | 403 => "credentials",
        408 | 429 | 500..=599 => "network",
        _ => "rejected",
    }
}

/// Indique si une chaîne d'erreurs évoque un échec TLS (reqwest n'expose aucun type dédié).
///
/// Paramètres : l'erreur de tête.
/// Retour : vrai si un maillon parle de certificat, de TLS ou de poignée de main.
fn mentions_tls(error: &(dyn Error + 'static)) -> bool {
    let mut current: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(link) = current {
        let text = link.to_string().to_lowercase();
        if ["certificate", "tls", "ssl", "handshake"]
            .iter()
            .any(|word| text.contains(word))
        {
            return true;
        }
        current = link.source();
    }
    false
}

/// Catégorie d'une erreur d'API du SDK.
///
/// Paramètres : l'erreur.
/// Retour : `tls`, `network` ou `rejected` pour un échec de transport, la catégorie du statut pour une réponse.
#[must_use]
pub fn api_category(error: &ApiError) -> &'static str {
    match error {
        ApiError::Reqwest(inner) if mentions_tls(inner) => "tls",
        ApiError::Reqwest(inner) if inner.is_connect() || inner.is_timeout() || inner.is_request() => "network",
        ApiError::ReqwestMiddleware(inner) if mentions_tls(inner) => "tls",
        ApiError::ReqwestMiddleware(_) | ApiError::Io(_) => "network",
        ApiError::Response(content) => status_category(content.status.as_u16()),
        ApiError::Reqwest(_) | ApiError::Serde(_) => "rejected",
    }
}

/// Catégorie d'un refus d'identité (`error` du corps de réponse de `/identity/connect/token`).
///
/// Paramètres : le champ `error` du serveur.
/// Retour : `credentials` pour `invalid_grant`, `invalid_username_or_password` et l'erreur vide de Vaultwarden, sinon `rejected`.
#[must_use]
pub fn identity_category(error: &str) -> &'static str {
    match error {
        "" | "invalid_grant" | "invalid_username_or_password" => "credentials",
        _ => "rejected",
    }
}

/// Code d'une erreur de connexion.
///
/// Paramètres : l'erreur du SDK.
/// Retour : `login:<catégorie>`.
#[must_use]
pub fn login_error(error: &LoginError) -> ErrorCode {
    let category = match error {
        LoginError::Api(ApiError::Response(content)) if content.status.as_u16() == 400 => "credentials",
        LoginError::Api(inner) | LoginError::Prelogin(PreloginError::Api(inner)) => api_category(inner),
        LoginError::IdentityFail(fail) => identity_category(&fail.error),
        LoginError::Crypto(_) | LoginError::EncryptionSettings(_) | LoginError::MasterPassword(_) => "credentials",
        _ => "rejected",
    };
    stage_error("login", category)
}

/// Code d'une erreur de synchronisation.
///
/// Paramètres : l'erreur du SDK.
/// Retour : `sync:<catégorie>`.
#[must_use]
pub fn sync_error(error: &SyncError) -> ErrorCode {
    let category = match error {
        SyncError::Api(inner) => api_category(inner),
        _ => "rejected",
    };
    stage_error("sync", category)
}

#[cfg(test)]
mod tests {
    use std::io;

    use bitwarden_api_api::ResponseContent;

    use super::*;

    #[test]
    fn statuses_map_to_categories() {
        assert_eq!(status_category(401), "credentials");
        assert_eq!(status_category(403), "credentials");
        assert_eq!(status_category(429), "network");
        assert_eq!(status_category(502), "network");
        assert_eq!(status_category(400), "rejected");
    }

    #[test]
    fn api_errors_are_classified_by_type() {
        let refused = ApiError::Io(io::Error::from(io::ErrorKind::ConnectionRefused));
        assert_eq!(api_category(&refused), "network");
        let unauthorized = ApiError::Response(ResponseContent {
            status: reqwest::StatusCode::UNAUTHORIZED,
            message: "refusé".into(),
        });
        assert_eq!(api_category(&unauthorized), "credentials");
        let malformed = ApiError::Serde(serde_json::from_str::<u8>("x").unwrap_err());
        assert_eq!(api_category(&malformed), "rejected");
    }

    #[test]
    fn identity_refusals_are_credentials() {
        assert_eq!(identity_category("invalid_grant"), "credentials");
        assert_eq!(identity_category("invalid_client"), "rejected");
    }

    #[test]
    fn tls_is_found_in_the_chain() {
        let tls = io::Error::other("invalid peer certificate: UnknownIssuer");
        assert!(mentions_tls(&tls));
        assert!(!mentions_tls(&io::Error::from(io::ErrorKind::ConnectionRefused)));
    }

    #[test]
    fn login_errors_become_stage_codes() {
        let refused = LoginError::Api(ApiError::Io(io::Error::from(io::ErrorKind::TimedOut)));
        assert_eq!(login_error(&refused).to_string(), "login:network");
        let bad = LoginError::Api(ApiError::Response(ResponseContent {
            status: reqwest::StatusCode::BAD_REQUEST,
            message: "refusé".into(),
        }));
        assert_eq!(login_error(&bad).to_string(), "login:credentials");
        assert_eq!(login_error(&LoginError::InvalidResponse).to_string(), "login:rejected");
    }
}
