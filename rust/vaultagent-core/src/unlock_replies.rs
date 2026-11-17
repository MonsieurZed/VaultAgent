//! Formes JSON publiques des demandes de déverrouillage, identiques à celles du protocole Node (sans clé `version`).

use crate::approval_policy::ApprovalPolicy;
use crate::device_record::{DeviceRecord, HelloCredential};
use serde::Serialize;

/// État d'une demande de déverrouillage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestState {
    /// En attente d'une décision.
    Pending,
    /// Déverrouillage en cours.
    Approving,
    /// Accordée, coffre ouvert.
    Approved,
    /// Refusée, consommée jusqu'à expiration.
    Denied,
    /// Déverrouillage échoué ou demande invalidée pendant l'ouverture.
    Failed,
}

/// Demande telle que la voit le client : `{vaultId, requestId, state, expiresUtc}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicRequest {
    /// Coffre concerné.
    pub vault_id: String,
    /// Identifiant UUID de la demande.
    pub request_id: String,
    /// État courant.
    pub state: RequestState,
    /// Expiration, ISO 8601 (`toISOString`).
    pub expires_utc: String,
    /// Présent et vrai quand l'acceptation automatique vient d'ouvrir le coffre.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlocked: Option<bool>,
}

/// Réponse quand le coffre est déjà ouvert.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlreadyUnlocked {
    /// Toujours `unlocked`.
    pub state: &'static str,
    /// Toujours vrai.
    pub unlocked: bool,
}

/// Réponse de `request-unlock`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum UnlockReply {
    /// Le coffre était déjà ouvert.
    Unlocked(AlreadyUnlocked),
    /// Demande créée, reprise ou consommée.
    Request(PublicRequest),
}

/// Demande en attente listée à l'administrateur ou à l'approbateur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedRequest {
    /// Coffre concerné.
    pub vault_id: String,
    /// Identifiant de la demande.
    pub request_id: String,
    /// État (toujours `pending` dans la liste).
    pub state: RequestState,
    /// Expiration ISO 8601.
    pub expires_utc: String,
    /// Défi à signer, base64 de 32 octets.
    pub challenge: String,
    /// Nom de l'appareil demandeur.
    pub device_name: Option<String>,
}

/// Défi d'un contexte signé (`reset-auto`, `enroll-elevation`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextReply {
    /// Coffre concerné.
    pub vault_id: String,
    /// Nom du contexte.
    pub request_id: String,
    /// Défi base64 de 32 octets.
    pub challenge: String,
}

/// Défi d'enrôlement Hello, avec le justificatif déjà enrôlé s'il y en a un.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloBegin {
    /// Coffre concerné.
    pub vault_id: String,
    /// Toujours `enroll-hello`.
    pub request_id: String,
    /// Défi base64 de 32 octets.
    pub challenge: String,
    /// Justificatif déjà enrôlé, `null` sinon.
    pub hello_credential: Option<HelloCredential>,
}

/// Réponse d'un enrôlement Hello réussi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloEnrolled {
    /// Toujours vrai.
    pub hello_enabled: bool,
    /// Toujours vrai : la politique exige désormais Hello.
    pub hello_required: bool,
    /// Coffre concerné.
    pub vault_id: String,
}

/// Réponse d'un enrôlement d'élévation réussi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevationEnrolled {
    /// Toujours vrai.
    pub elevation_enabled: bool,
    /// Coffre concerné.
    pub vault_id: String,
}

/// Réponse d'un retrait de clé d'élévation réussi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevationDisabled {
    /// Toujours faux.
    pub elevation_enabled: bool,
    /// Coffre concerné.
    pub vault_id: String,
}

/// Réponse de `reset-auto` : l'acceptation automatique est retirée à tous les appareils.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomaticReset {
    /// Toujours faux.
    pub automatic_acceptance: bool,
}

/// Réponse de `decide-unlock`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum DecisionReply {
    /// Décision sur une demande de déverrouillage.
    Request(PublicRequest),
    /// Retrait de l'acceptation automatique.
    Reset(AutomaticReset),
}

/// Champs `approvalSecurity` de `status`.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalSecurity {
    /// L'appareil a une clé d'approbateur.
    pub registered: bool,
    /// L'appareil a enrôlé Hello.
    pub hello_enabled: bool,
    /// L'appareil a enrôlé une clé d'élévation.
    pub elevation_enabled: bool,
    /// L'appareil peut enrôler Hello.
    pub hello_enrollment_allowed: bool,
    /// La politique exige Hello pour tout le profil.
    pub hello_required_for_all: bool,
    /// Hello est exigé pour cet appareil.
    pub hello_required: bool,
}

/// Demande en attente vue par un approbateur dans `status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    /// Demande listée.
    #[serde(flatten)]
    pub request: ListedRequest,
    /// Nom du coffre.
    pub vault_name: String,
    /// Hello exigé pour approuver.
    pub hello_required: bool,
    /// Signature d'élévation exigée pour approuver.
    pub elevation_required: bool,
    /// Justificatif Hello de l'approbateur, `null` sinon.
    pub hello_credential: Option<HelloCredential>,
}

/// Champs de `status` propres aux demandes : sécurité d'approbation, demande du client, demandes à approuver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockStatus {
    /// Sécurité d'approbation de l'appareil.
    pub approval_security: ApprovalSecurity,
    /// Demande en cours du client, `null` sinon.
    pub unlock_request: Option<PublicRequest>,
    /// Demandes en attente, vide si l'appareil n'est pas approbateur.
    pub approval_requests: Vec<ApprovalRequest>,
}

/// Résultat d'une opération qui peut changer les appareils et la politique : l'appelant écrit les fichiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome<T> {
    /// Réponse publique.
    pub value: T,
    /// Nouvelle liste d'appareils à enregistrer, si elle a changé.
    pub devices: Option<Vec<DeviceRecord>>,
    /// Nouvelle politique à enregistrer, si elle a changé.
    pub policy: Option<ApprovalPolicy>,
}

impl<T> Outcome<T> {
    /// Réponse sans effet sur les fichiers.
    ///
    /// Paramètres : la réponse.
    /// Retour : l'issue sans mutation.
    pub fn plain(value: T) -> Self {
        Self {
            value,
            devices: None,
            policy: None,
        }
    }
}
