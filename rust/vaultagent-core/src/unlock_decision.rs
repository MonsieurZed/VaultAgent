//! Décision signée d'un approbateur et vue `status` des demandes : contrôle de la signature RSA-PSS, de la preuve
//! Windows Hello et de la signature d'élévation avant de décider ou de retirer l'acceptation automatique.

use crate::device_list::{clear_automatic, set_automatic};
use crate::entropy::Entropy;
use crate::error::{ErrorCode, Result};
use crate::hello::verify_hello;
use crate::session_control::SessionControl;
use crate::unlock_contexts::{approver_of, elevation_of};
use crate::unlock_inputs::SignedDecisionInput;
use crate::unlock_replies::{ApprovalRequest, AutomaticReset, DecisionReply, Outcome, RequestState, UnlockStatus};
use crate::unlock_requests::{Env, UnlockRequests, find_record};
use crate::{approver_key, decision_message};

/// Défi signé et appareil demandeur éventuel d'une décision.
struct Target {
    challenge: String,
    requester: Option<String>,
}

impl UnlockRequests {
    /// Retrouve le défi signé : contexte `reset-auto` de l'approbateur, ou demande en attente non expirée.
    fn decision_target<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        request_id: &str,
        env: &mut Env<'_, S, E>,
    ) -> Result<Target> {
        self.prune(env);
        let found = if request_id == "reset-auto" {
            self.live_context(device_id, env.now).map(|context| Target {
                challenge: context.challenge.clone(),
                requester: None,
            })
        } else {
            self.entries
                .iter()
                .find(|entry| entry.id == request_id && entry.state == RequestState::Pending && entry.expires > env.now)
                .map(|entry| Target {
                    challenge: entry.challenge.clone(),
                    requester: Some(entry.device_id.clone()),
                })
        };
        found.ok_or(ErrorCode::RequestExpired)
    }

    /// Décide d'une demande, ou retire l'acceptation automatique (`reset-auto`), sur signature de l'approbateur.
    ///
    /// Paramètres : l'appareil approbateur, le corps reçu, l'origine du service, l'environnement.
    /// Retour : la réponse, la liste d'appareils à enregistrer (`remember` active l'acceptation automatique du
    /// demandeur, `reset-auto` la retire à tous).
    /// # Errors
    ///
    /// `approver-required` sans clé d'approbateur ou avec un corps incomplet ; `request-expired` ; `approval-refused` ;
    /// `hello-required` ou `hello-invalid` quand Hello est exigé pour un accord ; `elevation-required` quand une clé
    /// d'élévation est enrôlée ; l'erreur d'ouverture de la session.
    pub fn signed_decision<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        input: &SignedDecisionInput,
        origin: &str,
        env: &mut Env<'_, S, E>,
    ) -> Result<Outcome<DecisionReply>> {
        let record = find_record(env, device_id);
        let key = record.and_then(approver_of).ok_or(ErrorCode::ApproverRequired)?;
        let (Some(approve), Some(remember), Some(signature)) =
            (input.approve, input.remember, input.signature.as_deref())
        else {
            return Err(ErrorCode::ApproverRequired);
        };
        let target = self.decision_target(device_id, &input.request_id, env)?;
        let message = decision_message(
            origin,
            &self.vault_id,
            &input.request_id,
            &target.challenge,
            approve,
            remember,
        );
        approver_key::verify_pss(key, &message, signature, ErrorCode::ApprovalRefused)?;
        let Some(requester) = target.requester else {
            self.contexts.remove(device_id);
            return Ok(Outcome {
                value: DecisionReply::Reset(AutomaticReset {
                    automatic_acceptance: false,
                }),
                devices: Some(clear_automatic(env.records)),
                policy: None,
            });
        };
        if approve {
            self.check_approver_proofs(device_id, input, &message, env)?;
        }
        let reply = self.decide(&input.request_id, approve, None, env)?;
        let devices = match (approve && remember, env.session.profile()) {
            (true, Some(profile)) => Some(set_automatic(env.records, &requester, &profile, true)?),
            _ => None,
        };
        Ok(Outcome {
            value: DecisionReply::Request(reply),
            devices,
            policy: None,
        })
    }

    /// Exige la preuve Hello (si Hello est requis) et la signature d'élévation (si une clé est enrôlée).
    fn check_approver_proofs<S: SessionControl, E: Entropy>(
        &self,
        device_id: &str,
        input: &SignedDecisionInput,
        message: &str,
        env: &Env<'_, S, E>,
    ) -> Result<()> {
        let record = find_record(env, device_id);
        if self.security(device_id, env).hello_required {
            verify_hello(
                record.and_then(|device| device.hello_credential.as_ref()),
                input.hello_proof.as_ref(),
                message,
            )?;
        }
        match record.and_then(elevation_of) {
            Some(key) => approver_key::verify_pss(
                key,
                message,
                input.elevation_signature.as_deref().unwrap_or(""),
                ErrorCode::ElevationRequired,
            ),
            None => Ok(()),
        }
    }

    /// Champs de `status` liés aux demandes, pour l'appareil authentifié.
    ///
    /// Paramètres : l'appareil, le nom du coffre, l'environnement.
    /// Retour : `approvalSecurity`, `unlockRequest` et `approvalRequests` (vide si l'appareil n'est pas approbateur).
    pub fn status<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        vault_name: &str,
        env: &mut Env<'_, S, E>,
    ) -> UnlockStatus {
        let security = self.security(device_id, env);
        let credential = find_record(env, device_id).and_then(|device| device.hello_credential.clone());
        let unlock_request = self.current(device_id, env);
        let approval_requests = if security.registered {
            self.list(env)
                .into_iter()
                .map(|request| ApprovalRequest {
                    request,
                    vault_name: vault_name.to_owned(),
                    hello_required: security.hello_required,
                    elevation_required: security.elevation_enabled,
                    hello_credential: credential.clone(),
                })
                .collect()
        } else {
            Vec::new()
        };
        UnlockStatus {
            approval_security: security,
            unlock_request,
            approval_requests,
        }
    }
}
