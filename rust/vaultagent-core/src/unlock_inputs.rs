//! Corps JSON reçus par les routes d'approbation : décision signée, enrôlement Hello, enrôlement d'élévation. Un
//! champ absent ou d'un autre type est refusé par la logique, jamais par la désérialisation.

use crate::device_record::HelloCredential;
use crate::hello::HelloProof;
use serde::Deserialize;

/// Corps de `decide-unlock` d'un approbateur.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SignedDecisionInput {
    /// Demande décidée, ou `reset-auto`.
    pub request_id: String,
    /// Accord ou refus.
    pub approve: Option<bool>,
    /// Souhait de mémoriser l'accord pour l'appareil demandeur.
    pub remember: Option<bool>,
    /// Signature RSA-PSS de l'approbateur, base64.
    pub signature: Option<String>,
    /// Preuve Windows Hello, si exigée.
    pub hello_proof: Option<HelloProof>,
    /// Signature de la clé d'élévation, si enrôlée.
    pub elevation_signature: Option<String>,
}

/// Corps de `hello-enroll`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HelloEnrollInput {
    /// Justificatif à enrôler.
    pub credential: Option<HelloCredential>,
    /// Preuve Hello portant sur le message de décision d'enrôlement.
    pub hello_proof: Option<HelloProof>,
    /// Signature RSA-PSS de l'approbateur, base64.
    pub signature: Option<String>,
}

/// Corps de `elevation-enroll`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ElevationEnrollInput {
    /// Clé publique d'élévation (SPKI RSA 2048 en base64).
    pub public_key: String,
    /// Signature RSA-PSS de l'approbateur, base64.
    pub signature: Option<String>,
    /// Signature de la clé d'élévation sur le même message, base64.
    pub elevation_signature: Option<String>,
}

/// Corps de `elevation-disable`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ElevationDisableInput {
    /// Signature RSA-PSS de l'approbateur, base64.
    pub signature: Option<String>,
    /// Signature de la clé d'élévation enrôlée sur le même message, base64.
    pub elevation_signature: Option<String>,
}
