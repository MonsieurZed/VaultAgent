//! Contextes signés des demandes de déverrouillage : défi `reset-auto`, sécurité d'approbation, enrôlements de
//! Windows Hello et de la clé d'élévation (60 s de validité, un seul usage).

use crate::approval_policy::ApprovalPolicy;
use crate::device_list::{authorize_hello_enrollment, enroll_elevation, enroll_hello};
use crate::device_record::DeviceRecord;
use crate::entropy::Entropy;
use crate::error::{ErrorCode, Result};
use crate::hello::verify_hello;
use crate::session_control::SessionControl;
use crate::unlock_inputs::{ElevationEnrollInput, HelloEnrollInput};
use crate::unlock_replies::{ApprovalSecurity, ContextReply, ElevationEnrolled, HelloBegin, HelloEnrolled, Outcome};
use crate::unlock_requests::{CONTEXT_LIFETIME_MS, Context, Env, UnlockRequests, find_record, new_challenge};
use crate::{approver_key, decision_message};

/// Clé d'approbateur renseignée (non vide) d'un appareil.
pub(crate) fn approver_of(record: &DeviceRecord) -> Option<&str> {
    record.approver_public_key.as_deref().filter(|key| !key.is_empty())
}

/// Clé d'élévation renseignée (non vide) d'un appareil.
pub(crate) fn elevation_of(record: &DeviceRecord) -> Option<&str> {
    record.elevation_public_key.as_deref().filter(|key| !key.is_empty())
}

impl UnlockRequests {
    /// Enregistre un défi de contexte sous une clé.
    fn open_context(&mut self, key: String, challenge: &str, now: u64) {
        let context = Context {
            challenge: challenge.to_owned(),
            expires: now + CONTEXT_LIFETIME_MS,
        };
        self.contexts.insert(key, context);
    }

    /// Défi non expiré d'un contexte.
    pub(crate) fn live_context(&self, key: &str, now: u64) -> Option<&Context> {
        self.contexts.get(key).filter(|context| context.expires > now)
    }

    /// Ouvre le contexte `reset-auto` d'un approbateur.
    ///
    /// Paramètres : l'appareil approbateur, l'environnement.
    /// Retour : `{vaultId, requestId:"reset-auto", challenge}`.
    /// # Errors
    ///
    /// `approver-required` si l'appareil n'a pas de clé d'approbateur ; `engine-entropy`.
    pub fn context<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        env: &mut Env<'_, S, E>,
    ) -> Result<ContextReply> {
        find_record(env, device_id)
            .and_then(approver_of)
            .ok_or(ErrorCode::ApproverRequired)?;
        let challenge = new_challenge(env.entropy)?;
        self.open_context(device_id.to_owned(), &challenge, env.now);
        Ok(self.context_reply("reset-auto", challenge))
    }

    /// Réponse portant le défi d'un contexte.
    fn context_reply(&self, request_id: &str, challenge: String) -> ContextReply {
        ContextReply {
            vault_id: self.vault_id.clone(),
            request_id: request_id.to_owned(),
            challenge,
        }
    }

    /// Sécurité d'approbation d'un appareil, telle que `status` la publie.
    ///
    /// Paramètres : l'appareil, l'environnement.
    /// Retour : les six indicateurs ; tous faux pour un appareil inconnu (sauf la politique du profil).
    #[must_use]
    pub fn security<S: SessionControl, E: Entropy>(&self, device_id: &str, env: &Env<'_, S, E>) -> ApprovalSecurity {
        let record = find_record(env, device_id);
        let for_all = env.policy.requires_hello_for(env.session.profile().as_deref());
        let enabled = record.is_some_and(|device| device.hello_credential.is_some());
        ApprovalSecurity {
            registered: record.and_then(approver_of).is_some(),
            hello_enabled: enabled,
            elevation_enabled: record.and_then(elevation_of).is_some(),
            hello_enrollment_allowed: record.is_some_and(|device| device.hello_enrollment_allowed == Some(true)),
            hello_required_for_all: for_all,
            hello_required: enabled || for_all,
        }
    }

    /// Refuse l'enrôlement Hello d'un nouvel appareil quand la politique l'exige sans autorisation.
    fn check_hello_authorization<S: SessionControl, E: Entropy>(
        &self,
        record: &DeviceRecord,
        env: &Env<'_, S, E>,
    ) -> Result<()> {
        let blocked = record.hello_credential.is_none()
            && self.security(&record.id, env).hello_required_for_all
            && record.hello_enrollment_allowed != Some(true);
        if blocked {
            Err(ErrorCode::HelloEnrollmentAuthorizationRequired)
        } else {
            Ok(())
        }
    }

    /// Ouvre l'enrôlement de Windows Hello.
    ///
    /// Paramètres : l'appareil approbateur, l'environnement.
    /// Retour : le défi et le justificatif déjà enrôlé (`null` sinon).
    /// # Errors
    ///
    /// `hello-enrollment-refused` sans clé d'approbateur ; `hello-enrollment-authorization-required` quand la
    /// politique exige Hello et que l'appareil n'est pas autorisé à l'enrôler ; `engine-entropy`.
    pub fn begin_hello<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        env: &mut Env<'_, S, E>,
    ) -> Result<HelloBegin> {
        let record = find_record(env, device_id)
            .filter(|device| approver_of(device).is_some())
            .ok_or(ErrorCode::HelloEnrollmentRefused)?;
        self.check_hello_authorization(record, env)?;
        let challenge = new_challenge(env.entropy)?;
        self.open_context(format!("hello:{device_id}"), &challenge, env.now);
        Ok(HelloBegin {
            vault_id: self.vault_id.clone(),
            request_id: "enroll-hello".to_owned(),
            challenge,
            hello_credential: record.hello_credential.clone(),
        })
    }

    /// Termine l'enrôlement de Windows Hello : signature de l'approbateur et preuve Hello sur le même message.
    ///
    /// Paramètres : l'appareil, le corps reçu, l'origine du service, l'environnement.
    /// Retour : la réponse, la liste d'appareils à enregistrer (premier enrôlement) et la politique `requireHello`
    /// à enregistrer ; la session est verrouillée.
    /// # Errors
    ///
    /// `hello-enrollment-refused` (contexte absent ou expiré, justificatif différent de celui déjà enrôlé),
    /// `hello-enrollment-authorization-required`, `approval-refused`, `hello-required`, `hello-invalid`.
    pub fn complete_hello_enrollment<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        input: &HelloEnrollInput,
        origin: &str,
        env: &mut Env<'_, S, E>,
    ) -> Result<Outcome<HelloEnrolled>> {
        let record = find_record(env, device_id);
        let key = format!("hello:{device_id}");
        let challenge = self
            .live_context(&key, env.now)
            .map(|context| context.challenge.clone());
        let (Some(record), Some(challenge), Some(profile)) = (record, challenge, env.session.profile()) else {
            return Err(ErrorCode::HelloEnrollmentRefused);
        };
        let approver = approver_of(record).ok_or(ErrorCode::HelloEnrollmentRefused)?;
        self.check_hello_authorization(record, env)?;
        if let Some(enrolled) = &record.hello_credential
            && input.credential.as_ref() != Some(enrolled)
        {
            return Err(ErrorCode::HelloEnrollmentRefused);
        }
        let message = decision_message(origin, &self.vault_id, "enroll-hello", &challenge, true, false);
        let signature = input.signature.as_deref().unwrap_or("");
        approver_key::verify_pss(approver, &message, signature, ErrorCode::ApprovalRefused)?;
        verify_hello(input.credential.as_ref(), input.hello_proof.as_ref(), &message)?;
        self.contexts.remove(&key);
        let authorized = match record.hello_credential {
            None => Some(authorize_hello_enrollment(env.records, device_id, &profile)?),
            Some(_) => None,
        };
        env.session.lock();
        let devices = match (authorized, input.credential.clone()) {
            (Some(list), Some(credential)) => Some(enroll_hello(&list, device_id, &profile, credential)?),
            _ => None,
        };
        Ok(Outcome {
            value: HelloEnrolled {
                hello_enabled: true,
                hello_required: true,
                vault_id: self.vault_id.clone(),
            },
            devices,
            policy: Some(ApprovalPolicy::requiring_hello(&profile)),
        })
    }

    /// Ouvre l'enrôlement de la clé d'élévation.
    ///
    /// Paramètres : l'appareil approbateur, l'environnement.
    /// Retour : `{vaultId, requestId:"enroll-elevation", challenge}`.
    /// # Errors
    ///
    /// `elevation-enrollment-refused` sans clé d'approbateur ou si une clé d'élévation existe déjà ; `engine-entropy`.
    pub fn begin_elevation<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        env: &mut Env<'_, S, E>,
    ) -> Result<ContextReply> {
        let open = find_record(env, device_id)
            .is_some_and(|device| approver_of(device).is_some() && elevation_of(device).is_none());
        if !open {
            return Err(ErrorCode::ElevationEnrollmentRefused);
        }
        let challenge = new_challenge(env.entropy)?;
        self.open_context(format!("elevation:{device_id}"), &challenge, env.now);
        Ok(self.context_reply("enroll-elevation", challenge))
    }

    /// Termine l'enrôlement de la clé d'élévation : l'approbateur et la nouvelle clé signent le même message.
    ///
    /// Paramètres : l'appareil, le corps reçu, l'origine du service, l'environnement.
    /// Retour : la réponse et la liste d'appareils à enregistrer.
    /// # Errors
    ///
    /// `elevation-enrollment-refused` (contexte absent ou expiré, clé déjà enrôlée), `approval-refused`,
    /// `invalid-key`, `elevation-required`.
    pub fn complete_elevation_enrollment<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        input: &ElevationEnrollInput,
        origin: &str,
        env: &mut Env<'_, S, E>,
    ) -> Result<Outcome<ElevationEnrolled>> {
        let record = find_record(env, device_id);
        let key = format!("elevation:{device_id}");
        let challenge = self
            .live_context(&key, env.now)
            .map(|context| context.challenge.clone());
        let (Some(record), Some(challenge), Some(profile)) = (record, challenge, env.session.profile()) else {
            return Err(ErrorCode::ElevationEnrollmentRefused);
        };
        let approver = approver_of(record).ok_or(ErrorCode::ElevationEnrollmentRefused)?;
        if elevation_of(record).is_some() {
            return Err(ErrorCode::ElevationEnrollmentRefused);
        }
        let message = decision_message(origin, &self.vault_id, "enroll-elevation", &challenge, true, false);
        let signature = input.signature.as_deref().unwrap_or("");
        approver_key::verify_pss(approver, &message, signature, ErrorCode::ApprovalRefused)?;
        approver_key::rsa_public_key(&input.public_key)?;
        let proof = input.elevation_signature.as_deref().unwrap_or("");
        approver_key::verify_pss(&input.public_key, &message, proof, ErrorCode::ElevationRequired)?;
        self.contexts.remove(&key);
        let devices = enroll_elevation(env.records, device_id, &profile, &input.public_key)?;
        Ok(Outcome {
            value: ElevationEnrolled {
                elevation_enabled: true,
                vault_id: self.vault_id.clone(),
            },
            devices: Some(devices),
            policy: None,
        })
    }
}
