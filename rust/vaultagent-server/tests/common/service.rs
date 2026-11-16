//! Service complet de test : coffres sur faux moteur, horloge et attente factices, client associé, envoi de requêtes.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use vaultagent_core::devices::enroll;
use vaultagent_core::{PairRequests, SeededEntropy};
use vaultagent_engine::{FakeFactory, FakeHandle, Item, Login, LoginUri, Reply, Secret};
use vaultagent_server::{Context, ManualClock, RecordedPause, Request, Response, SharedEntropy, Store, Vaults, route};

use super::{TempDir, WALL_BASE, input};

pub const ORIGIN: &str = "https://vaultagent.example.test";
pub const OWNER: &str = "proprietaire";
pub const ITEM_ID: &str = "00000000-0000-4000-8000-0000000000a1";
pub const FOLDER_ID: &str = "00000000-0000-4000-8000-0000000000c1";
pub const REVISION: &str = "2026-11-08T00:00:00.000Z";
pub const NEXT_REVISION: &str = "2026-11-08T00:01:00.000Z";

/// État du coffre au départ d'un essai.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VaultState {
    Unconfigured,
    Locked,
    Unlocked,
}

/// Type de client associé au départ d'un essai.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Client {
    None,
    Paired,
    Approver,
}

/// Service de test.
pub struct Service {
    pub dir: TempDir,
    pub vaults: Arc<Vaults>,
    pub engine: FakeHandle,
    pub factory: FakeFactory,
    pub context: Context,
    pub pause: Arc<RecordedPause>,
    pub clock: Arc<ManualClock>,
    pub token: Option<String>,
}

impl Service {
    /// Construit le service avec un coffre `default` nommé « Coffre ».
    pub fn new(state: VaultState, client: Client) -> Self {
        let dir = TempDir::new("service");
        let root = Arc::new(Store::open(&dir.path().join("data")).unwrap());
        let factory = FakeFactory::new();
        let clock = Arc::new(ManualClock::new(WALL_BASE));
        let pause = Arc::new(RecordedPause::new());
        let entropy = SharedEntropy::new(Box::new(SeededEntropy::new(5)));
        let vaults = Arc::new(Vaults::open(root, Arc::new(factory.clone()), clock.clone(), entropy.clone()).unwrap());
        vaults.save_name(Some("default"), Some("Coffre")).unwrap();
        let engine = factory.engine();
        engine.push_ok();
        let vault = vaults.get(None).unwrap();
        if state != VaultState::Unconfigured {
            vaults.configure(&vault, &input(json!(15))).unwrap();
        }
        if state == VaultState::Locked {
            vault.session.lock();
        }
        let token = (client != Client::None && state != VaultState::Unconfigured).then(|| {
            let profile = vault.store.account().unwrap().profile;
            let key = (client == Client::Approver).then_some("approver-public-key-factice");
            let name = if key.is_some() {
                "Approbateur factice"
            } else {
                "Client factice"
            };
            let (records, enrolled) = enroll(
                name,
                &profile,
                key,
                WALL_BASE,
                &vault.store.devices(),
                &mut SeededEntropy::new(11),
            )
            .unwrap();
            vault.store.update_devices(|_| Ok(records)).unwrap();
            enrolled.token.to_string()
        });
        let context = Context {
            vaults: Arc::clone(&vaults),
            origin: ORIGIN.to_owned(),
            trusted_proxy: "127.0.0.1".to_owned(),
            owner: Some(OWNER.to_owned()),
            clock: clock.clone(),
            entropy,
            pause: pause.clone(),
            pair_requests: Arc::new(Mutex::new(PairRequests::new())),
            downloads_directory: None,
        };
        Self {
            dir,
            vaults,
            engine,
            factory,
            context,
            pause,
            clock,
            token,
        }
    }

    /// Empile les réponses du faux moteur.
    pub fn script(&self, replies: Vec<Reply>) {
        for reply in replies {
            self.engine.push(reply);
        }
    }

    /// Requête agent authentifiée avec un corps JSON.
    pub fn request(&self, route_name: &str, body: &Value) -> Request {
        let request = Request::new("POST", &format!("/api/agent/{route_name}"), "::ffff:127.0.0.1")
            .with_header("content-type", "application/json")
            .with_body(body.to_string().as_bytes());
        match &self.token {
            Some(token) => request.with_header("authorization", &format!("Bearer {token}")),
            None => request,
        }
    }

    /// Envoie une requête au cœur HTTP.
    pub fn send(&self, request: &Request) -> Response {
        route(request, &self.context)
    }

    /// Appelle une route agent et rend le statut et le corps JSON.
    pub fn call(&self, route_name: &str, body: &Value) -> (u16, Value) {
        let response = self.send(&self.request(route_name, body));
        (response.status, response.body_json().unwrap())
    }
}

/// Fiche de connexion factice, avec une URI de connexion.
pub fn demo_item(revision: &str, name: &str) -> Item {
    Item {
        id: ITEM_ID.to_owned(),
        name: name.to_owned(),
        kind: 1,
        revision_date: revision.to_owned(),
        login: Some(Login {
            username: Some("compte@example.invalid".to_owned()),
            password: Some(Secret::from("Fictif-42!")),
            totp: Some(Secret::from("FAKESEED")),
            uris: vec![LoginUri {
                uri: "https://example.test/login".to_owned(),
            }],
        }),
        ..Item::default()
    }
}

/// Réponse « fiche » du faux moteur.
pub fn item_reply(item: Item) -> Reply {
    Reply::Item(Box::new(item))
}
