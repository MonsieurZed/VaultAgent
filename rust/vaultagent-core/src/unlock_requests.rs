//! Demandes de déverrouillage (`unlock-requests.mjs`) : un client demande, un approbateur ou l'administrateur décide.
//! Logique pure : session, appareils, politique, heure et aléa arrivent par [`Env`] ; les mutations d'appareils sont
//! rendues à l'appelant. Les appareils sont un instantané : la révocation se détecte par la génération de session.

use crate::approval_policy::ApprovalPolicy;
use crate::device_record::{DeviceRecord, iso_from_millis};
use crate::device_token::random_uuid;
use crate::encoding::base64_encode;
use crate::entropy::Entropy;
use crate::error::{ErrorCode, Result};
use crate::session_control::SessionControl;
use crate::unlock_replies::{AlreadyUnlocked, ListedRequest, PublicRequest, RequestState, UnlockReply};
use std::collections::HashMap;

/// Durée de vie d'une demande de déverrouillage, en millisecondes.
pub const REQUEST_LIFETIME_MS: u64 = 300_000;
/// Durée de vie d'un défi de contexte (`reset-auto`, enrôlements), en millisecondes.
pub const CONTEXT_LIFETIME_MS: u64 = 60_000;
const CHALLENGE_BYTES: usize = 32;

/// Éléments extérieurs d'un appel : session, aléa, heure, appareils et politique du moment.
pub struct Env<'a, S: SessionControl, E: Entropy> {
    /// Session du coffre.
    pub session: &'a mut S,
    /// Source d'aléa injectée.
    pub entropy: &'a mut E,
    /// Heure en millisecondes Unix.
    pub now: u64,
    /// Appareils enregistrés, tous profils confondus.
    pub records: &'a [DeviceRecord],
    /// Politique `approval-policy.json` lue par l'appelant.
    pub policy: &'a ApprovalPolicy,
}

pub(crate) struct Entry {
    pub(crate) id: String,
    pub(crate) device_id: String,
    profile: String,
    generation: u64,
    pub(crate) expires: u64,
    pub(crate) state: RequestState,
    pub(crate) challenge: String,
}

pub(crate) struct Context {
    pub(crate) challenge: String,
    pub(crate) expires: u64,
}

/// Demandes et contextes signés en mémoire d'un coffre.
pub struct UnlockRequests {
    pub(crate) vault_id: String,
    pub(crate) entries: Vec<Entry>,
    pub(crate) contexts: HashMap<String, Context>,
}

/// Appareil du profil courant, s'il existe.
pub(crate) fn find_record<'a, S: SessionControl, E: Entropy>(
    env: &Env<'a, S, E>,
    device_id: &str,
) -> Option<&'a DeviceRecord> {
    let profile = env.session.profile();
    let records: &'a [DeviceRecord] = env.records;
    records
        .iter()
        .find(|device| device.id == device_id && Some(&device.profile) == profile.as_ref())
}

/// Tire un défi de 32 octets en base64.
pub(crate) fn new_challenge(entropy: &mut impl Entropy) -> Result<String> {
    let mut bytes = [0u8; CHALLENGE_BYTES];
    entropy.fill(&mut bytes)?;
    Ok(base64_encode(&bytes))
}

/// Une demande est valide si elle n'est pas expirée, si la session, le profil et l'appareil n'ont pas changé.
fn is_valid<S: SessionControl, E: Entropy>(entry: &Entry, env: &mut Env<'_, S, E>) -> bool {
    env.session.unlocked();
    entry.expires > env.now
        && entry.generation == env.session.generation()
        && Some(&entry.profile) == env.session.profile().as_ref()
        && find_record(env, &entry.device_id).is_some()
}

/// Déverrouille la session puis revérifie la demande ; verrouille de nouveau si elle est devenue invalide.
fn finish_unlock<S: SessionControl, E: Entropy>(
    entry: &Entry,
    code: Option<&str>,
    env: &mut Env<'_, S, E>,
) -> Result<()> {
    env.session.unlock(code)?;
    if is_valid(entry, env) {
        Ok(())
    } else {
        env.session.lock();
        Err(ErrorCode::RequestExpired)
    }
}

impl UnlockRequests {
    /// Crée un registre vide.
    ///
    /// Paramètres : l'identifiant du coffre, repris dans toutes les réponses.
    /// Retour : le registre.
    #[must_use]
    pub fn new(vault_id: &str) -> Self {
        Self {
            vault_id: vault_id.to_owned(),
            entries: Vec::new(),
            contexts: HashMap::new(),
        }
    }

    /// Supprime les demandes devenues invalides.
    pub(crate) fn prune<S: SessionControl, E: Entropy>(&mut self, env: &mut Env<'_, S, E>) {
        let entries = std::mem::take(&mut self.entries);
        self.entries = entries.into_iter().filter(|entry| is_valid(entry, env)).collect();
    }

    /// Forme publique de la demande à une position.
    fn public(&self, index: usize) -> PublicRequest {
        let entry = &self.entries[index];
        PublicRequest {
            vault_id: self.vault_id.clone(),
            request_id: entry.id.clone(),
            state: entry.state,
            expires_utc: iso_from_millis(entry.expires),
            unlocked: None,
        }
    }

    /// Ajoute une demande en attente pour un appareil.
    fn open_entry<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        profile: String,
        env: &mut Env<'_, S, E>,
    ) -> Result<usize> {
        let entry = Entry {
            id: random_uuid(env.entropy)?,
            device_id: device_id.to_owned(),
            profile,
            generation: env.session.generation(),
            expires: env.now + REQUEST_LIFETIME_MS,
            state: RequestState::Pending,
            challenge: new_challenge(env.entropy)?,
        };
        self.entries.push(entry);
        Ok(self.entries.len() - 1)
    }

    /// Demande en cours d'un appareil.
    ///
    /// Paramètres : l'identifiant de l'appareil, l'environnement.
    /// Retour : la demande publique, ou `None`.
    pub fn current<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        env: &mut Env<'_, S, E>,
    ) -> Option<PublicRequest> {
        self.prune(env);
        let index = self.entries.iter().position(|entry| entry.device_id == device_id)?;
        Some(self.public(index))
    }

    /// Crée une demande, ou accepte automatiquement si l'appareil le permet.
    ///
    /// Paramètres : l'appareil demandeur, l'environnement.
    /// Retour : `unlocked` si le coffre est déjà ouvert, sinon la demande (avec `unlocked:true` après acceptation
    /// automatique) ; une demande déjà traitée est rendue telle quelle jusqu'à expiration.
    /// # Errors
    ///
    /// `not-configured` sans compte ; `unauthorized` si l'appareil n'est plus associé ; `request-expired` si la
    /// demande devient invalide pendant l'ouverture ; l'erreur d'ouverture de la session ; `engine-entropy`.
    pub fn request<S: SessionControl, E: Entropy>(
        &mut self,
        device_id: &str,
        env: &mut Env<'_, S, E>,
    ) -> Result<UnlockReply> {
        self.prune(env);
        if env.session.unlocked() {
            return Ok(UnlockReply::Unlocked(AlreadyUnlocked {
                state: "unlocked",
                unlocked: true,
            }));
        }
        let profile = env.session.profile().ok_or(ErrorCode::NotConfigured)?;
        let record = find_record(env, device_id);
        let automatic = record.is_some_and(|device| device.auto_unlock == Some(true));
        let existing = self.entries.iter().position(|entry| entry.device_id == device_id);
        if let Some(index) = existing
            && (self.entries[index].state != RequestState::Pending || !automatic)
        {
            return Ok(UnlockReply::Request(self.public(index)));
        }
        if record.is_none() {
            return Err(ErrorCode::Unauthorized);
        }
        let index = match existing {
            Some(index) => index,
            None => self.open_entry(device_id, profile, env)?,
        };
        if !automatic {
            return Ok(UnlockReply::Request(self.public(index)));
        }
        self.entries[index].state = RequestState::Approving;
        let result = finish_unlock(&self.entries[index], None, env);
        self.entries[index].state = if result.is_ok() {
            RequestState::Approved
        } else {
            RequestState::Failed
        };
        result?;
        let mut reply = self.public(index);
        reply.unlocked = Some(true);
        Ok(UnlockReply::Request(reply))
    }

    /// Demandes en attente, vide si le coffre est déjà ouvert.
    ///
    /// Paramètres : l'environnement.
    /// Retour : les demandes `pending` avec leur défi et le nom de l'appareil.
    pub fn list<S: SessionControl, E: Entropy>(&mut self, env: &mut Env<'_, S, E>) -> Vec<ListedRequest> {
        if env.session.unlocked() {
            return Vec::new();
        }
        self.prune(env);
        self.entries
            .iter()
            .filter(|entry| entry.state == RequestState::Pending)
            .map(|entry| ListedRequest {
                vault_id: self.vault_id.clone(),
                request_id: entry.id.clone(),
                state: entry.state,
                expires_utc: iso_from_millis(entry.expires),
                challenge: entry.challenge.clone(),
                device_name: find_record(env, &entry.device_id).map(|device| device.name.clone()),
            })
            .collect()
    }

    /// Décide d'une demande en attente (administrateur, ou approbateur après contrôle des signatures).
    ///
    /// Paramètres : l'identifiant de la demande, accord ou refus, le code de double authentification éventuel,
    /// l'environnement.
    /// Retour : la demande dans son nouvel état (`approved`, `denied`).
    /// # Errors
    ///
    /// `request-expired` si la demande est inconnue, déjà traitée, expirée ou invalidée pendant l'ouverture
    /// (le coffre est alors reverrouillé) ; l'erreur d'ouverture de la session (la demande passe à `failed`).
    pub fn decide<S: SessionControl, E: Entropy>(
        &mut self,
        request_id: &str,
        approve: bool,
        code: Option<&str>,
        env: &mut Env<'_, S, E>,
    ) -> Result<PublicRequest> {
        self.prune(env);
        let index = self
            .entries
            .iter()
            .position(|entry| entry.id == request_id && entry.state == RequestState::Pending)
            .ok_or(ErrorCode::RequestExpired)?;
        if !approve {
            self.entries[index].state = RequestState::Denied;
            return Ok(self.public(index));
        }
        self.entries[index].state = RequestState::Approving;
        let result = finish_unlock(&self.entries[index], code, env);
        self.entries[index].state = if result.is_ok() {
            RequestState::Approved
        } else {
            RequestState::Failed
        };
        result.map(|()| self.public(index))
    }
}
