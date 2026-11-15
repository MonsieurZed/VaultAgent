//! Aides communes aux tests des demandes de déverrouillage : fausse session, monde déterministe (heure et aléa
//! injectés) et signatures RSA-PSS ; les preuves Hello EC2 sont dans `authenticator`.

#![allow(
    dead_code,
    unused_imports,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

pub mod authenticator;
pub use authenticator::{Authenticator, challenge_of, hex_bytes};

use rsa::RsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::pss::SigningKey as PssKey;
use rsa::signature::RandomizedSigner;
use serde_json::Value;
use sha2::Sha256;
use vaultagent_core::encoding::{base64_decode, base64_encode};
use vaultagent_core::{
    ApprovalPolicy, DeviceRecord, ElevationEnrollInput, EntropyRng, Env, ErrorCode, HelloEnrollInput, Outcome,
    PublicRequest, SeededEntropy, SessionControl, SignedDecisionInput, UnlockReply, UnlockRequests, decision_message,
};

pub const PROFILE: &str = "profil-factice";
pub const ORIGIN: &str = "https://vaultagent.example.test";
pub const VAULT: &str = "default";

/// Fixtures `unlock.json` produites par Node.
pub fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/unlock.json")).unwrap()
}

/// Texte d'un champ de fixture.
pub fn text(value: &Value) -> String {
    value.as_str().unwrap().to_owned()
}

/// Fausse session : compte les ouvertures, mémorise les codes, peut échouer ou être verrouillée pendant l'ouverture.
pub struct FakeSession {
    pub opened: bool,
    pub generation: u64,
    pub profile: Option<String>,
    pub unlocks: u32,
    pub codes: Vec<Option<String>>,
    pub fail_unlock: bool,
    pub lock_during_unlock: bool,
}

impl SessionControl for FakeSession {
    /// Implémentation de test de la méthode du même nom.
    fn unlocked(&mut self) -> bool {
        self.opened
    }

    /// Implémentation de test de la méthode du même nom.
    fn generation(&self) -> u64 {
        self.generation
    }

    /// Implémentation de test de la méthode du même nom.
    fn profile(&self) -> Option<String> {
        self.profile.clone()
    }

    /// Implémentation de test de la méthode du même nom.
    fn unlock(&mut self, code: Option<&str>) -> vaultagent_core::Result<()> {
        self.codes.push(code.map(str::to_owned));
        if self.fail_unlock {
            return Err(ErrorCode::Locked);
        }
        self.opened = true;
        self.unlocks += 1;
        if self.lock_during_unlock {
            self.lock();
        }
        Ok(())
    }

    /// Implémentation de test de la méthode du même nom.
    fn lock(&mut self) {
        self.opened = false;
        self.generation += 1;
    }
}

/// Appareil factice du profil, avec une clé d'approbateur facultative.
pub fn device(id: &str, name: &str, approver: Option<&str>) -> DeviceRecord {
    DeviceRecord {
        id: id.to_owned(),
        name: name.to_owned(),
        profile: PROFILE.to_owned(),
        created: "2026-01-01T00:00:00.000Z".to_owned(),
        hash: "empreinte-factice".to_owned(),
        approver_public_key: approver.map(str::to_owned),
        hello_enrollment_allowed: approver.map(|_| true),
        hello_credential: None,
        elevation_public_key: None,
        auto_unlock: None,
        extra: serde_json::Map::new(),
    }
}

/// Monde de test : session, file de demandes, appareils, politique, heure et aléa.
pub struct World {
    pub session: FakeSession,
    pub queue: UnlockRequests,
    pub entropy: SeededEntropy,
    pub records: Vec<DeviceRecord>,
    pub policy: ApprovalPolicy,
    pub now: u64,
}

impl World {
    /// Monde verrouillé et configuré avec les appareils donnés.
    pub fn new(records: Vec<DeviceRecord>) -> Self {
        Self {
            session: FakeSession {
                opened: false,
                generation: 0,
                profile: Some(PROFILE.to_owned()),
                unlocks: 0,
                codes: Vec::new(),
                fail_unlock: false,
                lock_during_unlock: false,
            },
            queue: UnlockRequests::new(VAULT),
            entropy: SeededEntropy::new(5),
            records,
            policy: ApprovalPolicy::default(),
            now: 0,
        }
    }

    /// Sépare la file de l'environnement qui emprunte le reste du monde.
    pub fn parts(&mut self) -> (&mut UnlockRequests, Env<'_, FakeSession, SeededEntropy>) {
        let env = Env {
            session: &mut self.session,
            entropy: &mut self.entropy,
            now: self.now,
            records: &self.records,
            policy: &self.policy,
        };
        (&mut self.queue, env)
    }

    /// Applique les mutations d'une issue (appareils, politique) et rend la réponse.
    pub fn apply<T>(&mut self, outcome: Outcome<T>) -> T {
        if let Some(devices) = outcome.devices {
            self.records = devices;
        }
        if let Some(policy) = outcome.policy {
            self.policy = policy;
        }
        outcome.value
    }

    /// `request-unlock` d'un appareil.
    pub fn request(&mut self, id: &str) -> vaultagent_core::Result<UnlockReply> {
        let (queue, mut env) = self.parts();
        queue.request(id, &mut env)
    }

    /// Demande en attente d'un appareil, en JSON, avec son identifiant.
    pub fn pending(&mut self, id: &str) -> PublicRequest {
        match self.request(id).unwrap() {
            UnlockReply::Request(request) => request,
            UnlockReply::Unlocked(_) => panic!("coffre déjà ouvert"),
        }
    }

    /// Demande courante d'un appareil.
    pub fn current(&mut self, id: &str) -> Option<PublicRequest> {
        let (queue, mut env) = self.parts();
        queue.current(id, &mut env)
    }

    /// Défi de la première demande en attente.
    pub fn challenge(&mut self) -> String {
        let (queue, mut env) = self.parts();
        queue.list(&mut env)[0].challenge.clone()
    }

    /// Décision de l'administrateur.
    pub fn decide(
        &mut self,
        request_id: &str,
        approve: bool,
        code: Option<&str>,
    ) -> vaultagent_core::Result<PublicRequest> {
        let (queue, mut env) = self.parts();
        queue.decide(request_id, approve, code, &mut env)
    }

    /// Décision signée d'un approbateur, mutations appliquées.
    pub fn signed(&mut self, id: &str, input: SignedDecisionInput) -> vaultagent_core::Result<Value> {
        let (queue, mut env) = self.parts();
        let outcome = queue.signed_decision(id, &input, ORIGIN, &mut env)?;
        Ok(serde_json::to_value(self.apply(outcome)).unwrap())
    }

    /// Fin d'un enrôlement Hello, mutations appliquées.
    pub fn enroll_hello(&mut self, id: &str, input: HelloEnrollInput) -> vaultagent_core::Result<Value> {
        let (queue, mut env) = self.parts();
        let outcome = queue.complete_hello_enrollment(id, &input, ORIGIN, &mut env)?;
        Ok(serde_json::to_value(self.apply(outcome)).unwrap())
    }

    /// Fin d'un enrôlement d'élévation, mutations appliquées.
    pub fn enroll_elevation(&mut self, id: &str, input: ElevationEnrollInput) -> vaultagent_core::Result<Value> {
        let (queue, mut env) = self.parts();
        let outcome = queue.complete_elevation_enrollment(id, &input, ORIGIN, &mut env)?;
        Ok(serde_json::to_value(self.apply(outcome)).unwrap())
    }
}

/// Clé privée RSA d'une fixture (PKCS#8 DER en base64).
pub fn rsa_private(pkcs8_base64: &str) -> RsaPrivateKey {
    RsaPrivateKey::from_pkcs8_der(&base64_decode(pkcs8_base64).unwrap()).unwrap()
}

/// Signature RSA-PSS SHA-256 de sel 32 en base64.
pub fn sign_pss(key: &RsaPrivateKey, message: &str) -> String {
    let mut source = SeededEntropy::new(99);
    let mut rng = EntropyRng::new(&mut source);
    let signature = PssKey::<Sha256>::new_with_salt_len(key.clone(), 32).sign_with_rng(&mut rng, message.as_bytes());
    base64_encode(&rsa::signature::SignatureEncoding::to_bytes(&signature))
}

/// Corps signé d'une décision d'approbateur.
pub fn decision(
    key: &RsaPrivateKey,
    request_id: &str,
    challenge: &str,
    approve: bool,
    remember: bool,
) -> SignedDecisionInput {
    let message = decision_message(ORIGIN, VAULT, request_id, challenge, approve, remember);
    SignedDecisionInput {
        request_id: request_id.to_owned(),
        approve: Some(approve),
        remember: Some(remember),
        signature: Some(sign_pss(key, &message)),
        hello_proof: None,
        elevation_signature: None,
    }
}

/// Message de décision de l'origine de test.
pub fn message(request_id: &str, challenge: &str, approve: bool, remember: bool) -> String {
    decision_message(ORIGIN, VAULT, request_id, challenge, approve, remember)
}
