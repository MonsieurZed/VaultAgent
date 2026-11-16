//! Tests de la session : chiffrement du mot de passe, délai glissant, verrouillage et génération, `configure`,
//! `unlock`, annulation pendant la connexion.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};

use common::{EMAIL, FICTITIOUS, Rig, SERVER, WALL_BASE, connect_call, connectable, input};
use serde_json::{Value, json};
use vaultagent_core::{DeviceRecord, ErrorCode, SessionControl, device_record::iso_from_millis};
use vaultagent_engine::{Engine, EngineFactory, FakeFactory, Secret};
use vaultagent_server::{ServerError, Session};

/// Erreur `locked` attendue.
fn locked_error() -> ServerError {
    ServerError::Code(ErrorCode::Locked)
}

#[test]
fn password_is_encrypted_on_disk_and_absent_from_status() {
    let rig = Rig::new("secret");
    rig.engine();
    rig.session.configure(&input(json!(15))).unwrap();
    let text = std::fs::read_to_string(rig.store.directory().join("account.json")).unwrap();
    assert!(!text.contains(FICTITIOUS));
    let account = rig.store.account().unwrap();
    assert_eq!(rig.store.unprotect(&account.password).unwrap().as_str(), FICTITIOUS);
    let status = serde_json::to_string(&rig.session.status()).unwrap();
    assert!(!status.contains(FICTITIOUS));
    assert!(status.contains("\"version\":\"3.0.0-preview.7\""));
}

#[test]
fn deadline_slides_status_does_not_extend_and_none_disables_it() {
    let rig = Rig::new("deadline");
    rig.engine();
    rig.session.configure(&input(json!(1))).unwrap();
    let first = rig.session.status();
    assert_eq!(first.expires_utc, Some(iso_from_millis(WALL_BASE + 60_000)));
    rig.clock.advance(50_000);
    assert_eq!(
        rig.session.status().expires_utc,
        Some(iso_from_millis(WALL_BASE + 60_000))
    );
    rig.clock.advance(11_000);
    let expired = rig.session.status();
    assert!(!expired.unlocked && expired.expires_utc.is_none());

    rig.engine();
    rig.session.unlock(None).unwrap();
    rig.clock.advance(50_000);
    let generation = rig.session.generation();
    rig.session.access(generation).unwrap();
    rig.session.touch();
    rig.clock.advance(50_000);
    assert!(rig.session.status().unlocked);
    rig.clock.advance(11_000);
    assert!(!rig.session.status().unlocked);

    rig.engine();
    rig.session.configure(&input(Value::Null)).unwrap();
    rig.clock.advance(1_000_000_000);
    let open = rig.session.status();
    assert!(open.unlocked && open.expires_utc.is_none() && open.timeout_minutes.is_none());
}

#[test]
fn lock_kills_the_engine_and_invalidates_an_operation_in_progress() {
    let rig = Rig::new("lock");
    let first = rig.engine();
    rig.session.configure(&input(json!(15))).unwrap();
    let generation = rig.session.generation();
    let slot = rig.session.access(generation).unwrap();
    rig.session.lock();
    assert_eq!(first.kills(), 1);
    assert_eq!(rig.session.access(generation).err(), Some(locked_error()));
    assert_eq!(slot.run(|engine| engine.sync()).err(), Some(locked_error()));

    rig.engine();
    rig.session.unlock(None).unwrap();
    assert!(rig.session.status().unlocked);
    assert_eq!(rig.session.access(generation).err(), Some(locked_error()));
    assert!(rig.session.access(rig.session.generation()).is_ok());
}

#[test]
fn dead_engine_locks_the_session() {
    let rig = Rig::new("dead");
    let handle = rig.engine();
    rig.session.configure(&input(json!(15))).unwrap();
    handle.set_alive(false);
    assert!(!rig.session.status().unlocked);
    assert_eq!(rig.session.access(rig.session.generation()).err(), Some(locked_error()));
}

#[test]
fn configure_validates_account_timeout_and_password() {
    let rig = Rig::new("validate");
    let mut bad = input(json!(15));
    bad.server = "http://vault.example.test".to_owned();
    assert_eq!(rig.session.configure(&bad).err(), Some(ServerError::InvalidAccount));
    bad.server = "https://vault.example.test/path".to_owned();
    assert_eq!(rig.session.configure(&bad).err(), Some(ServerError::InvalidAccount));
    bad.server = SERVER.to_owned();
    bad.email = "pas un courriel".to_owned();
    assert_eq!(rig.session.configure(&bad).err(), Some(ServerError::InvalidAccount));
    for timeout in [json!(0), json!(1441), json!(1.5), json!("15")] {
        assert_eq!(
            rig.session.configure(&input(timeout)).err(),
            Some(ServerError::InvalidTimeout)
        );
    }
    let mut absent = input(json!(15));
    absent.timeout_minutes = None;
    assert_eq!(rig.session.configure(&absent).err(), Some(ServerError::InvalidTimeout));
    let mut no_password = input(json!(15));
    no_password.password = None;
    assert_eq!(
        rig.session.configure(&no_password).err(),
        Some(ServerError::PasswordRequired)
    );
    assert_eq!(rig.factory.starts(), []);
    assert_eq!(input(json!(1440.0)).timeout().unwrap(), Some(1440));
    assert_eq!(input(json!(1)).timeout().unwrap(), Some(1));
}

/// Appareil factice du profil donné.
fn paired_device(profile: &str) -> DeviceRecord {
    serde_json::from_value(json!({
        "id": "22222222-2222-4222-8222-222222222222", "name": "Client factice", "profile": profile,
        "created": "2026-11-01T10:00:00.000Z", "hash": "0"
    }))
    .unwrap()
}

#[test]
fn configure_keeps_profile_for_same_account_and_resets_it_for_a_new_one() {
    let rig = Rig::new("profile");
    rig.engine();
    rig.session.configure(&input(json!(15))).unwrap();
    let profile = rig.store.account().unwrap().profile;
    rig.store.update_devices(|_| Ok(vec![paired_device(&profile)])).unwrap();

    let handle = rig.engine();
    let mut same = input(json!(30));
    same.password = None;
    same.email = EMAIL.to_uppercase();
    rig.session.configure(&same).unwrap();
    let account = rig.store.account().unwrap();
    assert_eq!(account.profile, profile);
    assert_eq!(account.timeout_minutes, Some(30));
    assert_eq!(rig.store.devices().len(), 1);
    assert_eq!(connect_call(&handle), (Secret::from(FICTITIOUS), None));

    rig.engine();
    let mut other = input(json!(30));
    other.email = "autre@example.invalid".to_owned();
    rig.session.configure(&other).unwrap();
    assert_ne!(rig.store.account().unwrap().profile, profile);
    assert_eq!(rig.store.devices(), []);
    assert_eq!(reopened_device_count(rig.store.directory()), 0);
}

/// Nombre d'appareils lus par une réouverture du dossier.
fn reopened_device_count(directory: &Path) -> usize {
    vaultagent_server::Store::open(directory).unwrap().devices().len()
}

#[test]
fn configure_replaces_the_running_engine() {
    let rig = Rig::new("replace");
    let old = rig.engine();
    rig.session.configure(&input(json!(15))).unwrap();
    let generation = rig.session.generation();
    rig.engine();
    rig.session.configure(&input(json!(15))).unwrap();
    assert_eq!(old.kills(), 1);
    assert!(rig.session.generation() > generation);
    assert!(rig.session.status().unlocked);
}

#[test]
fn unlock_needs_a_configured_account_and_reuses_the_stored_password() {
    let rig = Rig::new("unlock");
    assert_eq!(
        rig.session.unlock(None).err(),
        Some(ServerError::Code(ErrorCode::NotConfigured))
    );
    rig.engine();
    rig.session.configure(&input(json!(15))).unwrap();
    rig.session.lock();
    let handle = rig.engine();
    let code = zeroize::Zeroizing::new("123456".to_owned());
    rig.session.unlock(Some(&code)).unwrap();
    assert_eq!(
        connect_call(&handle),
        (Secret::from(FICTITIOUS), Some(Secret::from("123456")))
    );
    assert!(rig.session.unlock(None).unwrap().unlocked);
}

#[test]
fn failed_unlock_stays_locked_and_kills_nothing_running() {
    let rig = Rig::new("failunlock");
    rig.engine();
    rig.session.configure(&input(json!(15))).unwrap();
    rig.session.lock();
    rig.factory.push_failure("engine-init");
    assert_eq!(
        rig.session.unlock(None).err().map(|error| error.to_string()),
        Some("engine-init".to_owned())
    );
    assert!(!rig.session.status().unlocked);
}

#[test]
fn failed_connection_kills_the_engine_and_relays_the_code() {
    let rig = Rig::new("failconnect");
    let handle = rig.factory.engine();
    handle.push_error(vaultagent_engine::stage_error("login", "credentials"));
    assert_eq!(
        rig.session
            .configure(&input(json!(15)))
            .err()
            .map(|error| error.to_string()),
        Some("login:credentials".to_owned())
    );
    assert_eq!(handle.kills(), 1);
    assert_eq!(handle.call_names(), ["connect"]);
    assert!(rig.store.account().is_none() && !rig.session.status().unlocked);
}

struct LockingFactory {
    inner: FakeFactory,
    session: Mutex<Option<Arc<Session>>>,
}

impl EngineFactory for LockingFactory {
    /// Verrouille la session pendant le démarrage du moteur, puis délègue à la fabrique factice.
    fn start(&self, directory: &Path, profile: &str) -> vaultagent_core::Result<Box<dyn Engine>> {
        if let Some(session) = self.session.lock().unwrap().as_ref() {
            session.lock();
        }
        self.inner.start(directory, profile)
    }
}

#[test]
fn configure_is_cancelled_when_the_session_is_locked_during_connection() {
    let dir = common::TempDir::new("cancel");
    let store = Arc::new(vaultagent_server::Store::open(&dir.path().join("data")).unwrap());
    let inner = FakeFactory::new();
    let handle = connectable(&inner);
    let factory = Arc::new(LockingFactory {
        inner,
        session: Mutex::new(None),
    });
    let session = Arc::new(Session::new(
        Arc::clone(&store),
        factory.clone(),
        Arc::new(vaultagent_server::ManualClock::new(WALL_BASE)),
        vaultagent_server::SharedEntropy::system(),
    ));
    *factory.session.lock().unwrap() = Some(Arc::clone(&session));
    assert_eq!(session.configure(&input(json!(15))).err(), Some(ServerError::Cancelled));
    assert_eq!(handle.kills(), 1);
    assert!(store.account().is_none());
    assert!(!dir.path().join("data").join("account.json").exists());
}

#[test]
fn session_control_drives_the_same_session() {
    let rig = Rig::new("control");
    rig.engine();
    rig.session.configure(&input(json!(15))).unwrap();
    let mut control = &*rig.session;
    assert!(control.unlocked());
    assert_eq!(control.profile(), rig.store.account().map(|account| account.profile));
    let before = SessionControl::generation(&control);
    control.lock();
    assert!(SessionControl::generation(&control) > before);
    assert!(!control.unlocked());
    rig.engine();
    SessionControl::unlock(&mut control, Some("654321")).unwrap();
    assert!(control.unlocked());
}
