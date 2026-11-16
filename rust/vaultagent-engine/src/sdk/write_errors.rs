//! Classement des erreurs d'écriture du SDK (création, édition, corbeille, restauration, partage) en codes
//! `<étape>:<catégorie>` ; un refus « out of date » de Vaultwarden devient `revision-conflict`. Aucun message du
//! serveur n'entre dans le code rendu.

use bitwarden_api_api::ApiError;
use bitwarden_vault::{CipherError, CreateCipherError, DeleteCipherError, EditCipherError, RestoreCipherError};
use vaultagent_core::ErrorCode;

use crate::engine::stage_error;
use crate::sdk::errors::api_category;

/// Indique si une erreur d'API est le refus de révision périmée de Vaultwarden (400 « out of date »).
///
/// Paramètres : l'erreur d'API.
/// Retour : vrai pour un statut 400 dont le corps parle d'une fiche « out of date ».
#[must_use]
pub fn is_out_of_date(error: &ApiError) -> bool {
    matches!(error, ApiError::Response(content)
        if content.status.as_u16() == 400 && content.message.to_lowercase().contains("out of date"))
}

/// Code d'une erreur d'API pendant une écriture.
///
/// Paramètres : l'étape du protocole et l'erreur.
/// Retour : `revision-conflict` pour une révision périmée, sinon `<étape>:<catégorie>`.
fn api_code(stage: &str, error: &ApiError) -> ErrorCode {
    if is_out_of_date(error) {
        ErrorCode::RevisionConflict
    } else {
        stage_error(stage, api_category(error))
    }
}

/// Code d'une erreur de création de fiche.
///
/// Paramètres : l'étape (`write` ou `passkeys`) et l'erreur du SDK.
/// Retour : le code classé.
#[must_use]
pub fn create_error(stage: &str, error: &CreateCipherError) -> ErrorCode {
    match error {
        CreateCipherError::Api(inner) => api_code(stage, inner),
        _ => stage_error(stage, "rejected"),
    }
}

/// Code d'une erreur de modification de fiche.
///
/// Paramètres : l'erreur du SDK.
/// Retour : `revision-conflict` ou `write:<catégorie>`.
#[must_use]
pub fn edit_error(error: &EditCipherError) -> ErrorCode {
    match error {
        EditCipherError::Api(inner) => api_code("write", inner),
        _ => stage_error("write", "rejected"),
    }
}

/// Code d'une erreur de mise à la corbeille.
///
/// Paramètres : l'erreur du SDK.
/// Retour : `write:<catégorie>`.
#[must_use]
pub fn delete_error(error: &DeleteCipherError) -> ErrorCode {
    match error {
        DeleteCipherError::Api(inner) => api_code("write", inner),
        DeleteCipherError::Repository(_) => stage_error("write", "rejected"),
    }
}

/// Code d'une erreur de restauration.
///
/// Paramètres : l'erreur du SDK.
/// Retour : `write:<catégorie>`.
#[must_use]
pub fn restore_error(error: &RestoreCipherError) -> ErrorCode {
    match error {
        RestoreCipherError::Api(inner) => api_code("write", inner),
        _ => stage_error("write", "rejected"),
    }
}

/// Code d'une erreur de partage.
///
/// Paramètres : l'erreur du SDK.
/// Retour : `sharing:refused` si la fiche appartient déjà à une organisation, sinon `sharing:<catégorie>`.
#[must_use]
pub fn share_error(error: &CipherError) -> ErrorCode {
    match error {
        CipherError::OrganizationAlreadySet => stage_error("sharing", "refused"),
        CipherError::Api(inner) => stage_error("sharing", api_category(inner)),
        _ => stage_error("sharing", "rejected"),
    }
}

#[cfg(test)]
mod tests {
    use bitwarden_api_api::ResponseContent;

    use super::*;

    /// Erreur d'API de réponse factice.
    fn response(status: u16, message: &str) -> ApiError {
        ApiError::Response(ResponseContent {
            status: reqwest::StatusCode::from_u16(status).unwrap_or(reqwest::StatusCode::BAD_REQUEST),
            message: message.into(),
        })
    }

    #[test]
    fn out_of_date_is_only_a_400_with_that_text() {
        assert!(is_out_of_date(&response(
            400,
            "{\"message\":\"The client copy of this cipher is out of date.\"}"
        )));
        assert!(!is_out_of_date(&response(400, "autre refus")));
        assert!(!is_out_of_date(&response(500, "out of date")));
    }

    #[test]
    fn edit_errors_are_classified_without_the_server_message() {
        let conflict = EditCipherError::Api(response(400, "Out Of Date"));
        assert_eq!(edit_error(&conflict), ErrorCode::RevisionConflict);
        let denied = EditCipherError::Api(response(403, "interdit secret"));
        assert_eq!(edit_error(&denied).to_string(), "write:credentials");
        let down = EditCipherError::Api(response(503, "x"));
        assert_eq!(edit_error(&down).to_string(), "write:network");
        assert_eq!(edit_error(&EditCipherError::GatedCipher).to_string(), "write:rejected");
    }

    #[test]
    fn create_delete_restore_and_share_errors_have_their_stage() {
        let bad = || response(400, "refus");
        assert_eq!(
            create_error("passkeys", &CreateCipherError::Api(bad())).to_string(),
            "passkeys:rejected"
        );
        assert_eq!(
            delete_error(&DeleteCipherError::Api(bad())).to_string(),
            "write:rejected"
        );
        assert_eq!(
            restore_error(&RestoreCipherError::Api(response(429, ""))).to_string(),
            "write:network"
        );
        assert_eq!(
            share_error(&CipherError::OrganizationAlreadySet).to_string(),
            "sharing:refused"
        );
        assert_eq!(
            share_error(&CipherError::Api(response(403, ""))).to_string(),
            "sharing:credentials"
        );
    }
}
