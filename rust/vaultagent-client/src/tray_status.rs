//! État du serveur vu par l'application de barre : coffres, déverrouillage, validation UAC et demandes en attente
//! avec le nom du client, du coffre et l'échéance.

use serde::Deserialize;
use serde_json::Value;

use crate::api::{VaultBody, agent_call};
use crate::approval_request::ApprovalRequest;
use crate::elevation_enroll::{VaultEntry, decode, flag};
use crate::error::ClientError;
use crate::iso_time::parse_unix_seconds;
use crate::token::Token;
use crate::transport::Transport;

/// Demande de déverrouillage avec ce que la carte affiche.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TrayRequest {
    /// Demande telle que la décision la signe.
    #[serde(flatten)]
    pub request: ApprovalRequest,
    /// Nom du client qui demande l'accès.
    #[serde(rename = "deviceName", default)]
    pub device_name: String,
    /// Nom du coffre demandé.
    #[serde(rename = "vaultName", default)]
    pub vault_name: Option<String>,
    /// Échéance ISO 8601.
    #[serde(rename = "expiresUtc", default)]
    pub expires_utc: Option<String>,
}

impl TrayRequest {
    /// Échéance de la demande.
    ///
    /// Retour : les secondes Unix, ou `None` si le serveur n'en donne pas ou si elle est illisible.
    #[must_use]
    pub fn expires_at(&self) -> Option<i64> {
        self.expires_utc.as_deref().and_then(parse_unix_seconds)
    }

    /// Indique si la demande est échue.
    ///
    /// Paramètres : l'heure courante en secondes Unix.
    /// Retour : vrai si l'échéance est connue et passée ; une demande sans échéance n'est jamais échue.
    #[must_use]
    pub fn is_expired(&self, now: i64) -> bool {
        self.expires_at().is_some_and(|end| end <= now)
    }

    /// Identifiant de notification, unique parmi les coffres.
    ///
    /// Retour : `coffre/demande`.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}/{}", self.request.vault(), self.request.request_id)
    }
}

/// État d'un coffre.
#[derive(Debug, Clone, PartialEq)]
pub struct VaultSnapshot {
    /// Identifiant du coffre.
    pub vault_id: String,
    /// Vrai si le coffre est déverrouillé pour les clients.
    pub unlocked: bool,
    /// Vrai si la validation UAC est active sur ce coffre.
    pub elevation_enabled: bool,
    /// Vrai si ce client est approbateur de ce coffre.
    pub registered: bool,
    /// Demandes en attente.
    pub requests: Vec<TrayRequest>,
}

/// Lit la réponse de `status` d'un coffre.
///
/// Paramètres : l'identifiant du coffre et le corps JSON.
/// Retour : l'état du coffre ; les demandes illisibles sont ignorées ; erreur si le corps n'est pas un objet JSON.
///
/// # Errors
///
/// `BadResponse` si le corps n'est pas du JSON.
pub fn parse_snapshot(vault_id: &str, status: &str) -> Result<VaultSnapshot, ClientError> {
    let value: Value = decode(status)?;
    let security = value.get("approvalSecurity").cloned().unwrap_or(Value::Null);
    let requests = value
        .get("approvalRequests")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |list| {
            list.iter()
                .filter_map(|item| serde_json::from_value::<TrayRequest>(item.clone()).ok())
                .map(|mut request| {
                    request.request.vault_id.get_or_insert_with(|| vault_id.to_owned());
                    request
                })
                .collect()
        });
    Ok(VaultSnapshot {
        vault_id: vault_id.to_owned(),
        unlocked: value.get("unlocked").and_then(Value::as_bool).unwrap_or(false),
        elevation_enabled: flag(&security, "elevationEnabled"),
        registered: flag(&security, "registered"),
        requests,
    })
}

/// Lit l'état de tous les coffres du jeton.
///
/// Paramètres : le transport et le jeton.
/// Retour : un état par coffre, dans l'ordre du serveur.
///
/// # Errors
///
/// Les erreurs possibles sont celles du transport et du serveur, classées par `agent_call`.
pub fn fetch_snapshots(transport: &dyn Transport, token: &Token) -> Result<Vec<VaultSnapshot>, ClientError> {
    let vaults: Vec<VaultEntry> = decode(&agent_call(transport, token, "vaults", &VaultBody { vault_id: None })?)?;
    vaults
        .iter()
        .map(|vault| {
            let body = VaultBody {
                vault_id: Some(&vault.id),
            };
            parse_snapshot(&vault.id, &agent_call(transport, token, "status", &body)?)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_transport::FakeTransport;
    use crate::test_dir::sample_token;

    const STATUS: &str = "{\"unlocked\":true,\"approvalSecurity\":{\"registered\":true,\"elevationEnabled\":true},\
        \"approvalRequests\":[{\"requestId\":\"r1\",\"challenge\":\"c\",\"deviceName\":\"PC\",\"vaultName\":\"Maison\",\
        \"expiresUtc\":\"1970-01-01T00:01:40Z\",\"helloRequired\":true},{\"broken\":1}]}";

    #[test]
    fn snapshot_carries_flags_and_card_fields() {
        let snapshot = parse_snapshot("v1", STATUS).unwrap();
        assert!(snapshot.unlocked && snapshot.elevation_enabled && snapshot.registered);
        assert_eq!(snapshot.requests.len(), 1);
        let request = &snapshot.requests[0];
        assert_eq!(request.request.vault(), "v1");
        assert_eq!(
            (request.device_name.as_str(), request.vault_name.as_deref()),
            ("PC", Some("Maison"))
        );
        assert!(request.request.hello_required);
        assert_eq!(request.key(), "v1/r1");
        assert_eq!(request.expires_at(), Some(100));
        assert!(request.is_expired(100) && !request.is_expired(99));
    }

    #[test]
    fn missing_security_means_locked_card_mode() {
        let snapshot = parse_snapshot("v", "{}").unwrap();
        assert!(!snapshot.unlocked && !snapshot.elevation_enabled && !snapshot.registered);
        assert_eq!(snapshot.requests.len(), 0);
        assert_eq!(parse_snapshot("v", "nope").unwrap_err().code(), "bad-response");
    }

    #[test]
    fn all_vaults_are_read_in_order() {
        let fake = FakeTransport::new()
            .reply(200, "[{\"id\":\"a\"},{\"id\":\"b\"}]")
            .reply(200, STATUS)
            .reply(200, "{\"unlocked\":false}");
        let token = Token::parse(&sample_token()).unwrap();
        let snapshots = fetch_snapshots(&fake, &token).unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!((snapshots[0].vault_id.as_str(), snapshots[1].unlocked), ("a", false));
        assert_eq!(fake.calls()[1].path, "/api/agent/status");
    }
}
