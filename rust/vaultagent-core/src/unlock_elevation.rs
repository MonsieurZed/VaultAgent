//! Retrait de la clé d'élévation : contexte `disable-elevation` puis preuve signée par la clé enrôlée, pour qu'un
//! agent ne puisse pas abaisser la sécurité sans validation UAC (60 s de validité, un seul usage).

use crate::device_list::disable_elevation;
use crate::entropy::Entropy;
use crate::error::{ErrorCode, Result};
use crate::session_control::SessionControl;
use crate::unlock_contexts::{approver_of, elevation_of};
use crate::unlock_inputs::ElevationDisableInput;
use crate::unlock_replies::{ContextReply, ElevationDisabled, Outcome};
use crate::unlock_requests::{Env, UnlockRequests, find_record, new_challenge};
use crate::{approver_key, decision_message};

const REQUEST_ID: &str = "disable-elevation";

impl UnlockRequests {
    /// Ouvre le retrait de la clé d'élévation.
    ///
    /// Paramètres : l'appareil approbateur, l'environnement.
    /// Retour : `{vaultId, requestId:"disable-elevation", challenge}`.
    /// # Errors
    ///
    /// `approver-required` sans clé d'approbateur ; `elevation-not-enrolled` sans clé d'élévation ; `engine-entropy`.
    pub fn begin_elevation_disable<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        env: &mut Env<'_, S, E>,
    ) -> Result<ContextReply> {
        let record = find_record(env, device_id)
            .filter(|device| approver_of(device).is_some())
            .ok_or(ErrorCode::ApproverRequired)?;
        if elevation_of(record).is_none() {
            return Err(ErrorCode::ElevationNotEnrolled);
        }
        let challenge = new_challenge(env.entropy)?;
        self.open_context(format!("disable-elevation:{device_id}"), &challenge, env.now);
        Ok(self.context_reply(REQUEST_ID, challenge))
    }

    /// Termine le retrait : l'approbateur et la clé d'élévation enrôlée signent le même message.
    ///
    /// Paramètres : l'appareil, le corps reçu, l'origine du service, l'environnement.
    /// Retour : la réponse et la liste d'appareils à enregistrer.
    /// # Errors
    ///
    /// Dans l'ordre : `approver-required`, `elevation-not-enrolled`, `request-expired` (contexte absent, expiré ou
    /// déjà utilisé), `approval-refused` (signature de l'approbateur ou de la clé d'élévation).
    pub fn complete_elevation_disable<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        input: &ElevationDisableInput,
        origin: &str,
        env: &mut Env<'_, S, E>,
    ) -> Result<Outcome<ElevationDisabled>> {
        let record = find_record(env, device_id);
        let approver = record.and_then(approver_of).ok_or(ErrorCode::ApproverRequired)?;
        let elevation = record.and_then(elevation_of).ok_or(ErrorCode::ElevationNotEnrolled)?;
        let key = format!("disable-elevation:{device_id}");
        let challenge = self
            .live_context(&key, env.now)
            .map(|context| context.challenge.clone())
            .ok_or(ErrorCode::RequestExpired)?;
        let profile = env.session.profile().ok_or(ErrorCode::NotConfigured)?;
        let message = decision_message(origin, &self.vault_id, REQUEST_ID, &challenge, true, false);
        let signature = input.signature.as_deref().unwrap_or("");
        approver_key::verify_pss(approver, &message, signature, ErrorCode::ApprovalRefused)?;
        let proof = input.elevation_signature.as_deref().unwrap_or("");
        approver_key::verify_pss(elevation, &message, proof, ErrorCode::ApprovalRefused)?;
        self.contexts.remove(&key);
        let devices = disable_elevation(env.records, device_id, &profile)?;
        Ok(Outcome {
            value: ElevationDisabled {
                elevation_enabled: false,
                vault_id: self.vault_id.clone(),
            },
            devices: Some(devices),
            policy: None,
        })
    }
}
