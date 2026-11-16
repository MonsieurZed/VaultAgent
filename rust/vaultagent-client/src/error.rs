//! Erreurs du client, leur code de protocole, leur code de sortie et la ligne JSON écrite sur stderr.

use serde::Serialize;

use crate::exit_code::ExitCode;

/// Version du format de la ligne d'erreur.
const ERROR_VERSION: u8 = 1;

/// Erreur du client ; aucun de ses textes ne contient de secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// Usage incorrect ou donnée invalide fournie par l'appelant.
    Usage(String),
    /// Configuration absente, illisible, non privée ou invalide.
    Config(String),
    /// Réponse d'erreur du serveur : statut HTTP et code du protocole.
    Server {
        /// Statut HTTP.
        status: u16,
        /// Code du protocole (`locked`, `vault-required`, `unauthorized`…).
        code: String,
    },
    /// Le serveur n'a pas pu être joint : rien n'a été envoyé.
    Unreachable(String),
    /// Délai ou coupure après l'envoi : le résultat est inconnu.
    Indeterminate,
    /// Écriture sans réponse après l'envoi : l'appelant garde l'`operationId` et relit avant de réessayer.
    WriteIndeterminate,
    /// Réponse du serveur invalide ou trop grosse.
    BadResponse(String),
    /// Demande d'association refusée par l'administrateur.
    PairingDenied,
    /// Demande d'association expirée sans réponse.
    PairingExpired,
}

/// Code du serveur pour une écriture dont le résultat est inconnu.
const WRITE_INDETERMINATE: &str = "write-indeterminate";

/// Conduite à tenir après une écriture indéterminée, sans rappeler aucune valeur.
const WRITE_ADVICE: &str = "Write outcome unknown: keep the operationId and read preview again before any retry.";

/// Ligne JSON d'erreur écrite sur stderr.
#[derive(Serialize)]
struct ErrorLine<'a> {
    version: u8,
    error: &'a str,
    message: &'a str,
}

impl ClientError {
    /// Code court de l'erreur : celui du protocole s'il existe, sinon un code propre au client.
    ///
    /// Retour : le code, sans secret.
    #[must_use]
    pub fn code(&self) -> &str {
        match self {
            Self::Usage(_) => "usage",
            Self::Config(_) => "config",
            Self::Server { code, .. } => code,
            Self::Unreachable(_) => "unreachable",
            Self::Indeterminate | Self::WriteIndeterminate => "indeterminate",
            Self::BadResponse(_) => "bad-response",
            Self::PairingDenied => "pairing-denied",
            Self::PairingExpired => "pairing-expired",
        }
    }

    /// Message court destiné à l'humain.
    ///
    /// Retour : le texte, sans secret.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Usage(text) | Self::Config(text) | Self::Unreachable(text) | Self::BadResponse(text) => text.clone(),
            Self::Server { code, .. } if code == WRITE_INDETERMINATE => WRITE_ADVICE.to_owned(),
            Self::WriteIndeterminate => WRITE_ADVICE.to_owned(),
            Self::Server { status, code } => format!("Server refused the request (HTTP {status}, {code})."),
            Self::Indeterminate => "No answer after sending; the outcome is unknown.".to_owned(),
            Self::PairingDenied => "Pairing request denied.".to_owned(),
            Self::PairingExpired => "Pairing request expired.".to_owned(),
        }
    }

    /// Code de sortie du processus pour cette erreur.
    ///
    /// Retour : 1 pour usage et configuration, 4 pour un coffre verrouillé (423), 5 pour un résultat indéterminé,
    /// 2 pour tout autre refus.
    #[must_use]
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage(_) | Self::Config(_) => ExitCode::Usage,
            Self::Server { status: 423, .. } => ExitCode::Locked,
            Self::Indeterminate | Self::WriteIndeterminate => ExitCode::Indeterminate,
            Self::Server { code, .. } if code == WRITE_INDETERMINATE => ExitCode::Indeterminate,
            Self::Server { .. }
            | Self::Unreachable(_)
            | Self::BadResponse(_)
            | Self::PairingDenied
            | Self::PairingExpired => ExitCode::Refused,
        }
    }

    /// Ligne JSON d'erreur, sans saut de ligne final.
    ///
    /// Retour : `{"version":1,"error":"<code>","message":"<texte>"}`.
    #[must_use]
    pub fn error_line(&self) -> String {
        let message = self.message();
        let line = ErrorLine {
            version: ERROR_VERSION,
            error: self.code(),
            message: &message,
        };
        serde_json::to_string(&line).unwrap_or_else(|_| {
            "{\"version\":1,\"error\":\"internal\",\"message\":\"Error line not encodable.\"}".to_owned()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_status_maps_to_exit_four() {
        let error = ClientError::Server {
            status: 423,
            code: "locked".to_owned(),
        };
        assert_eq!(error.exit_code(), ExitCode::Locked);
        assert_eq!(error.code(), "locked");
    }

    #[test]
    fn write_indeterminate_gives_exit_five_and_advice() {
        let server = ClientError::Server {
            status: 500,
            code: "write-indeterminate".to_owned(),
        };
        assert_eq!(server.exit_code(), ExitCode::Indeterminate);
        assert!(server.message().contains("operationId"));
        let wire = ClientError::WriteIndeterminate;
        assert_eq!(wire.exit_code(), ExitCode::Indeterminate);
        assert_eq!(wire.code(), "indeterminate");
        assert!(wire.message().contains("preview"));
    }

    #[test]
    fn other_statuses_map_to_refused() {
        for status in [400, 403, 500] {
            let error = ClientError::Server {
                status,
                code: "vault-required".to_owned(),
            };
            assert_eq!(error.exit_code(), ExitCode::Refused);
        }
    }

    #[test]
    fn indeterminate_and_usage_codes() {
        assert_eq!(ClientError::Indeterminate.exit_code(), ExitCode::Indeterminate);
        assert_eq!(ClientError::Usage("x".to_owned()).exit_code(), ExitCode::Usage);
        assert_eq!(ClientError::Config("x".to_owned()).exit_code(), ExitCode::Usage);
        assert_eq!(ClientError::PairingDenied.exit_code(), ExitCode::Refused);
    }

    #[test]
    fn error_line_is_one_versioned_json_line() {
        let line = ClientError::Server {
            status: 403,
            code: "unauthorized".to_owned(),
        }
        .error_line();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["error"], "unauthorized");
        assert!(value["message"].as_str().unwrap().contains("403"));
        assert!(!line.contains('\n'));
    }
}
