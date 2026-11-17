//! Demandes d'approbation en attente (`status.approvalRequests`) : lecture typée et recherche par identifiant.

use serde::Deserialize;
use serde_json::Value;

use crate::api::{VaultBody, agent_call};
use crate::error::ClientError;
use crate::token::Token;
use crate::transport::Transport;

/// Coffre supposé quand la demande n'en nomme pas.
pub const DEFAULT_VAULT: &str = "default";

/// Demande de déverrouillage à approuver, avec les exigences de preuve.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ApprovalRequest {
    /// Coffre concerné (`default` s'il est absent).
    #[serde(rename = "vaultId", default)]
    pub vault_id: Option<String>,
    /// Identifiant de la demande.
    #[serde(rename = "requestId")]
    pub request_id: String,
    /// Défi à signer.
    pub challenge: String,
    /// Une preuve Windows Hello est exigée.
    #[serde(rename = "helloRequired", default)]
    pub hello_required: bool,
    /// Une validation Windows (UAC) est exigée.
    #[serde(rename = "elevationRequired", default)]
    pub elevation_required: bool,
    /// Identifiant de clé Hello enrôlée, s'il y en a une.
    #[serde(rename = "helloCredential", default)]
    pub hello_credential: Option<Value>,
}

impl ApprovalRequest {
    /// Coffre de la demande.
    ///
    /// Retour : l'identifiant de coffre, ou `default`.
    #[must_use]
    pub fn vault(&self) -> &str {
        self.vault_id.as_deref().unwrap_or(DEFAULT_VAULT)
    }
}

/// Extrait les demandes d'une réponse `status`.
///
/// Paramètres : le corps JSON de `status`.
/// Retour : les demandes lisibles, dans l'ordre ; vide si le champ est absent.
#[must_use]
pub fn parse_requests(status: &str) -> Vec<ApprovalRequest> {
    serde_json::from_str::<Value>(status)
        .ok()
        .and_then(|value| value.get("approvalRequests").cloned())
        .and_then(|list| serde_json::from_value(list).ok())
        .unwrap_or_default()
}

/// Lit `status` et rend sa liste de demandes en attente.
///
/// Paramètres : le transport, le jeton et le coffre visé.
/// Retour : le corps `status` brut et les demandes ; l'erreur classée d'un appel sinon.
///
/// # Errors
///
/// Les erreurs possibles sont celles de `agent_call`.
pub fn fetch(
    transport: &dyn Transport,
    token: &Token,
    vault: Option<&str>,
) -> Result<Vec<ApprovalRequest>, ClientError> {
    agent_call(transport, token, "status", &VaultBody { vault_id: vault }).map(|body| parse_requests(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_read_with_defaults() {
        let body = "{\"approvalRequests\":[{\"requestId\":\"r\",\"challenge\":\"c\"},\
            {\"vaultId\":\"v\",\"requestId\":\"r2\",\"challenge\":\"c2\",\"helloRequired\":true,\
            \"elevationRequired\":true,\"helloCredential\":null}]}";
        let requests = parse_requests(body);
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].vault(), "default");
        assert!(!requests[0].hello_required && !requests[0].elevation_required);
        assert_eq!(requests[1].vault(), "v");
        assert!(requests[1].hello_required && requests[1].elevation_required);
    }

    #[test]
    fn missing_or_bad_list_is_empty() {
        assert_eq!(parse_requests("{}").len(), 0);
        assert_eq!(parse_requests("not json").len(), 0);
        assert_eq!(parse_requests("{\"approvalRequests\":[{\"x\":1}]}").len(), 0);
    }
}
