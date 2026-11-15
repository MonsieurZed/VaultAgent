//! Demandes d'association approuvées depuis la page d'administration (`pair-requests.mjs`) : le client reçoit un code
//! de contrôle `ABCD-EFGH` à comparer, puis attend l'approbation du propriétaire. Logique pure : heure et aléa injectés.

use crate::approver_key::rsa_public_key;
use crate::devices::Enrolled;
use crate::encoding::{base64url_encode, digest, same};
use crate::entropy::Entropy;
use crate::error::{ErrorCode, Result};
use crate::jsval::{trim, utf16_len};
use crate::pair_types::{Listed, Opened, Polled, Target};

/// Durée de vie d'une demande, en millisecondes.
pub const LIFETIME_MS: u64 = 300_000;
/// Demandes en attente (ou à relever) au plus.
pub const MAX_PENDING: usize = 8;
/// Ouvertures par minute au plus.
pub const MAX_OPENINGS_PER_MINUTE: usize = 10;
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const WINDOW_MS: u64 = 60_000;
const MAX_NAME_UNITS: usize = 80;

enum State {
    Pending,
    Approved(Polled),
    Denied,
}

struct Entry {
    id: String,
    name: String,
    verification: String,
    approver_public_key: Option<String>,
    secret_hash: String,
    state: State,
    expires: u64,
}

/// Demandes d'association en mémoire, dans l'ordre d'ouverture.
#[derive(Default)]
pub struct PairRequests {
    entries: Vec<Entry>,
    openings: Vec<u64>,
}

/// Secondes restantes arrondies à la seconde, jamais négatives.
fn seconds_left(expires: u64, now: u64) -> u64 {
    (expires.saturating_sub(now) + 500) / 1000
}

/// Retire les caractères de contrôle `U+0000`–`U+001F` et `U+007F`, puis les espaces de bord.
fn clean_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| *c >= ' ' && *c != '\u{7f}').collect();
    trim(&kept).to_owned()
}

impl PairRequests {
    /// Crée un registre vide.
    ///
    /// Paramètres : aucun.
    /// Retour : le registre.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Retire les demandes expirées.
    ///
    /// Paramètres : l'heure en millisecondes.
    /// Retour : rien.
    fn purge(&mut self, now: u64) {
        self.entries.retain(|entry| entry.expires >= now);
    }

    /// Trouve une demande encore en attente.
    ///
    /// Paramètres : l'identifiant.
    /// Retour : la demande.
    /// # Errors
    ///
    /// `request-expired` si elle est absente ou déjà décidée.
    fn pending_mut(&mut self, request_id: &str) -> Result<&mut Entry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.id == request_id && matches!(entry.state, State::Pending))
            .ok_or(ErrorCode::RequestExpired)
    }

    /// Ouvre une demande côté client, sans authentification.
    ///
    /// Paramètres : le nom proposé, la clé publique du PC approbateur (SPKI base64) si elle est fournie, l'heure en
    /// millisecondes, la source d'aléa.
    /// Retour : identifiant, secret, code de contrôle et durée.
    /// # Errors
    ///
    /// `invalid-device` (nom vide ou de plus de 80 unités après nettoyage) ; `invalid-key` ; `pairing-rate-limit`
    /// (10 ouvertures par minute) ; `pairing-limit` (8 demandes) ; `engine-entropy`.
    pub fn open(
        &mut self,
        name: &str,
        approver_public_key: Option<&str>,
        now: u64,
        entropy: &mut impl Entropy,
    ) -> Result<Opened> {
        let clean = clean_name(name);
        if clean.is_empty() || utf16_len(&clean) > MAX_NAME_UNITS {
            return Err(ErrorCode::InvalidDevice);
        }
        if let Some(key) = approver_public_key {
            rsa_public_key(key)?;
        }
        self.purge(now);
        self.openings.retain(|time| time + WINDOW_MS > now);
        if self.openings.len() >= MAX_OPENINGS_PER_MINUTE {
            return Err(ErrorCode::PairingRateLimit);
        }
        if self.entries.len() >= MAX_PENDING {
            return Err(ErrorCode::PairingLimit);
        }
        let verification = verification_code(entropy)?;
        let mut id = [0u8; 16];
        let mut secret = [0u8; 32];
        entropy.fill(&mut id)?;
        entropy.fill(&mut secret)?;
        let (request_id, secret) = (base64url_encode(&id), base64url_encode(&secret));
        self.openings.push(now);
        self.entries.push(Entry {
            id: request_id.clone(),
            name: clean,
            verification: verification.clone(),
            approver_public_key: approver_public_key.map(str::to_owned),
            secret_hash: digest(secret.as_bytes()),
            state: State::Pending,
            expires: now + LIFETIME_MS,
        });
        Ok(Opened {
            request_id,
            secret,
            verification,
            expires_in_seconds: LIFETIME_MS / 1000,
        })
    }

    /// Liste les demandes en attente pour la page d'administration.
    ///
    /// Paramètres : l'heure.
    /// Retour : les demandes ouvertes, dans l'ordre d'ouverture.
    pub fn list(&mut self, now: u64) -> Vec<Listed> {
        self.purge(now);
        self.entries
            .iter()
            .filter(|entry| matches!(entry.state, State::Pending))
            .map(|entry| Listed {
                request_id: entry.id.clone(),
                name: entry.name.clone(),
                verification: entry.verification.clone(),
                approver: entry.approver_public_key.is_some(),
                expires_in_seconds: seconds_left(entry.expires, now),
            })
            .collect()
    }

    /// Donne le nom et la clé à enrôler pour approuver une demande.
    ///
    /// Paramètres : l'identifiant, l'heure.
    /// Retour : le nom et la clé d'approbateur facultative.
    /// # Errors
    ///
    /// `request-expired` si la demande est absente, expirée ou déjà décidée.
    pub fn target(&mut self, request_id: &str, now: u64) -> Result<Target> {
        self.purge(now);
        let entry = self.pending_mut(request_id)?;
        Ok(Target {
            name: entry.name.clone(),
            approver_public_key: entry.approver_public_key.clone(),
        })
    }

    /// Approuve une demande avec le résultat de l'enrôlement ; le jeton sera remis au prochain sondage.
    ///
    /// Paramètres : l'identifiant, l'heure, le coffre qui reçoit l'appareil, le résultat de l'enrôlement.
    /// Retour : rien.
    /// # Errors
    ///
    /// `request-expired` si la demande est absente, expirée ou déjà décidée.
    pub fn approve(&mut self, request_id: &str, now: u64, vault_id: &str, enrolled: &Enrolled) -> Result<()> {
        self.purge(now);
        let entry = self.pending_mut(request_id)?;
        entry.state = State::Approved(Polled::Approved {
            token: (*enrolled.token).clone(),
            device_id: enrolled.device_id.clone(),
            vault_id: vault_id.to_owned(),
        });
        Ok(())
    }

    /// Refuse une demande en attente.
    ///
    /// Paramètres : l'identifiant.
    /// Retour : rien.
    /// # Errors
    ///
    /// `request-expired` si la demande est absente ou déjà décidée.
    pub fn deny(&mut self, request_id: &str) -> Result<()> {
        self.pending_mut(request_id)?.state = State::Denied;
        Ok(())
    }

    /// Lit l'état d'une demande côté client ; la décision n'est remise qu'une fois, la demande disparaît ensuite.
    ///
    /// Paramètres : l'identifiant, le secret remis à l'ouverture, l'heure.
    /// Retour : `pending`, `denied` ou `approved` avec le jeton.
    /// # Errors
    ///
    /// `pairing-refused` si l'identifiant est inconnu, expiré ou le secret faux.
    pub fn poll(&mut self, request_id: &str, secret: &str, now: u64) -> Result<Polled> {
        self.purge(now);
        let hash = digest(secret.as_bytes());
        let index = self
            .entries
            .iter()
            .position(|entry| entry.id == request_id && same(&entry.secret_hash, &hash))
            .ok_or(ErrorCode::PairingRefused)?;
        if matches!(self.entries[index].state, State::Pending) {
            return Ok(Polled::Pending {
                expires_in_seconds: seconds_left(self.entries[index].expires, now),
            });
        }
        Ok(match self.entries.remove(index).state {
            State::Approved(result) => result,
            _ => Polled::Denied,
        })
    }
}

/// Tire le code de contrôle `ABCD-EFGH` de l'alphabet sans caractères ambigus.
fn verification_code(entropy: &mut impl Entropy) -> Result<String> {
    let mut code = String::with_capacity(9);
    for index in 0..8 {
        if index == 4 {
            code.push('-');
        }
        let pick = usize::try_from(entropy.below(32)?).unwrap_or(0);
        code.push(char::from(ALPHABET[pick % ALPHABET.len()]));
    }
    Ok(code)
}
