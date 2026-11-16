//! Outils partagés des tests du serveur : dossier temporaire nettoyé, session sur faux moteur et horloge factice.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod front;
pub mod service;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::Value;
use vaultagent_core::SeededEntropy;
use vaultagent_engine::{Call, FakeFactory, FakeHandle, Secret};
use vaultagent_server::{ConfigureInput, ManualClock, Session, SharedEntropy, Store};
use zeroize::Zeroizing;

pub const FICTITIOUS: &str = "Fictif-essai-42!";
pub const SERVER: &str = "https://vault.example.test";
pub const EMAIL: &str = "probe@example.invalid";
pub const WALL_BASE: u64 = 1_800_000_000_000;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Dossier temporaire supprimé à la destruction.
pub struct TempDir(PathBuf);

impl TempDir {
    /// Crée un dossier neuf sous le dossier temporaire du système.
    pub fn new(label: &str) -> Self {
        let name = format!(
            "vaultagent-server-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    /// Chemin du dossier.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    /// Supprime le dossier et son contenu.
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Session prête à tester : magasin, faux moteurs, horloge factice.
pub struct Rig {
    pub dir: TempDir,
    pub store: Arc<Store>,
    pub factory: FakeFactory,
    pub clock: Arc<ManualClock>,
    pub session: Arc<Session>,
}

impl Rig {
    /// Magasin neuf dans `<temp>/data`.
    pub fn new(label: &str) -> Self {
        let dir = TempDir::new(label);
        let store = Arc::new(Store::open(&dir.path().join("data")).unwrap());
        Self::around(dir, store)
    }

    /// Session construite autour d'un magasin existant.
    pub fn around(dir: TempDir, store: Arc<Store>) -> Self {
        let factory = FakeFactory::new();
        let clock = Arc::new(ManualClock::new(WALL_BASE));
        let session = Arc::new(Session::new(
            Arc::clone(&store),
            Arc::new(factory.clone()),
            clock.clone(),
            SharedEntropy::new(Box::new(SeededEntropy::new(7))),
        ));
        Self {
            dir,
            store,
            factory,
            clock,
            session,
        }
    }

    /// Prépare un moteur qui accepte `connect`.
    pub fn engine(&self) -> FakeHandle {
        connectable(&self.factory)
    }
}

/// Prépare dans la fabrique un moteur qui accepte `connect`.
pub fn connectable(factory: &FakeFactory) -> FakeHandle {
    let handle = factory.engine();
    handle.push_ok();
    handle
}

/// Entrée de `configure` avec le compte factice, le délai donné (`null` pour désactiver).
pub fn input(timeout: Value) -> ConfigureInput {
    ConfigureInput {
        server: SERVER.to_owned(),
        email: EMAIL.to_owned(),
        password: Some(Zeroizing::new(FICTITIOUS.to_owned())),
        timeout_minutes: Some(timeout),
        code: None,
    }
}

/// Nom des fichiers d'un dossier, triés.
pub fn names(directory: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

/// Mot de passe et code reçus par le premier `connect` d'un faux moteur.
pub fn connect_call(handle: &FakeHandle) -> (Secret, Option<Secret>) {
    handle
        .calls()
        .into_iter()
        .find_map(|call| match call {
            Call::Connect { password, code, .. } => Some((password, code)),
            _ => None,
        })
        .unwrap()
}
