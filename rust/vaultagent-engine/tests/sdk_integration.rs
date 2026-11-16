//! Intégration du moteur natif contre un Vaultwarden jetable : connexion, sync, lectures, TOTP, dossiers,
//! organisations, contrôle du partage et verrouillage. Tous les tests sont `#[ignore]` et lisent
//! `VAULTAGENT_TEST_SERVER`, `VAULTAGENT_TEST_EMAIL` et `VAULTAGENT_TEST_PASSWORD` (compte factice).
//!
//! Lancement : `cargo test -p vaultagent-engine --test sdk_integration -- --ignored`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use vaultagent_core::Uuid;
use vaultagent_engine::{Account, Engine, EngineFactory, SdkEngineFactory};
use zeroize::Zeroizing;

/// Valeur d'une variable d'environnement obligatoire.
fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("variable {name} absente"))
}

/// Moteur neuf, non connecté.
fn fresh() -> Box<dyn Engine> {
    SdkEngineFactory
        .start(Path::new("absent"), "test")
        .expect("démarrage du moteur")
}

/// Connecte un moteur au compte factice de la configuration.
fn connected() -> Box<dyn Engine> {
    let mut engine = fresh();
    let server = env("VAULTAGENT_TEST_SERVER");
    let email = env("VAULTAGENT_TEST_EMAIL");
    let password = Zeroizing::new(env("VAULTAGENT_TEST_PASSWORD"));
    let account = Account {
        server: &server,
        email: &email,
    };
    engine.connect(&account, &password, None).expect("connexion");
    engine
}

/// Texte du code d'échec d'un résultat.
fn code<T>(result: vaultagent_core::Result<T>) -> String {
    result.err().map_or_else(String::new, |error| error.to_string())
}

/// Identifiant validé.
fn uuid(text: &str) -> Uuid {
    Uuid::parse(text).expect("uuid")
}

/// Suffixe unique des noms créés par les tests.
fn unique() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos())
}

#[test]
#[ignore = "exige un Vaultwarden de test"]
fn read_path_sync_list_item_search_and_totp() {
    let mut engine = connected();
    check_listing(engine.as_mut());
    check_item_and_search(engine.as_mut());
    check_totp(engine.as_mut());
}

/// Liste, resynchronise, relit la liste et la corbeille.
fn check_listing(engine: &mut dyn Engine) {
    let items = engine.items(None).expect("liste");
    assert!(!items.is_empty(), "le compte de test doit porter des fiches");
    assert!(
        items
            .iter()
            .all(|i| matches!(i.kind, 1 | 2) && i.deleted_date.is_none() && i.archived_date.is_none())
    );
    engine.sync().expect("resynchronisation");
    assert_eq!(engine.items(None).expect("liste").len(), items.len());
    assert!(
        engine
            .trashed_items()
            .expect("corbeille")
            .iter()
            .all(|i| i.deleted_date.is_some())
    );
}

/// Lit une fiche, la retrouve par recherche et vérifie l'absence d'une fiche inconnue.
fn check_item_and_search(engine: &mut dyn Engine) {
    let first = engine.items(None).expect("liste").remove(0);
    let read = engine.item(&uuid(&first.id)).expect("lecture");
    assert_eq!(
        (read.id.as_str(), read.name.as_str()),
        (first.id.as_str(), first.name.as_str())
    );
    assert_eq!(read.revision_date, first.revision_date);
    let found = engine.items(Some(&first.name)).expect("recherche");
    assert!(found.iter().any(|i| i.id == first.id));
    assert_eq!(
        engine
            .items(Some("absent-absent-absent-0001"))
            .expect("recherche")
            .len(),
        0
    );
    let missing = uuid("99999999-9999-4999-8999-999999999999");
    assert_eq!(code(engine.item(&missing)), "items:not-found");
}

/// Code TOTP d'une fiche à graine, refus `totp:missing` pour une fiche sans graine.
fn check_totp(engine: &mut dyn Engine) {
    let with_seed = engine
        .items(None)
        .expect("liste")
        .into_iter()
        .find(|i| i.login.as_ref().is_some_and(|l| l.totp.is_some()))
        .expect("une fiche du compte de test doit porter un TOTP");
    let generated = engine.totp(&uuid(&with_seed.id)).expect("totp");
    assert!(generated.expose().len() >= 6 && generated.expose().chars().all(|c| c.is_ascii_digit()));
    let without = engine
        .items(None)
        .expect("liste")
        .into_iter()
        .find(|i| i.login.as_ref().is_none_or(|l| l.totp.is_none()));
    if let Some(item) = without {
        assert_eq!(code(engine.totp(&uuid(&item.id))), "totp:missing");
    }
}

#[test]
#[ignore = "exige un Vaultwarden de test"]
fn folders_can_be_created_renamed_and_deleted() {
    let mut engine = connected();
    let name = format!("Test 2.1c {}", unique());
    let renamed = format!("{name} renommé");
    let id = engine.create_folder(&name).expect("création");
    let listed = engine.folders().expect("liste");
    assert!(listed.iter().any(|f| f.id == id && f.name == name));
    engine.rename_folder(&uuid(&id), &renamed).expect("renommage");
    assert!(
        engine
            .folders()
            .expect("liste")
            .iter()
            .any(|f| f.id == id && f.name == renamed)
    );
    engine.delete_folder(&uuid(&id)).expect("suppression");
    assert!(!engine.folders().expect("liste").iter().any(|f| f.id == id));
    assert_eq!(code(engine.rename_folder(&uuid(&id), "x")), "write:rejected");
}

#[test]
#[ignore = "exige un Vaultwarden de test"]
fn organizations_and_sharing_checks() {
    let mut engine = connected();
    let orgs = engine.organizations().expect("organisations");
    let org = orgs
        .iter()
        .find(|o| o.name == "Org factice" && o.collections.iter().any(|c| c.can_edit))
        .expect("une organisation factice avec une collection éditable");
    let editable: Vec<_> = org.collections.iter().filter(|c| c.can_edit).collect();
    assert!(
        !editable.is_empty(),
        "l'organisation de test doit avoir une collection éditable"
    );
    let org_id = uuid(&org.id);
    engine
        .check_sharing(&org_id, &[uuid(&editable[0].id)])
        .expect("partage permis");
    let unknown = uuid("99999999-9999-4999-8999-999999999999");
    assert_eq!(
        code(engine.check_sharing(&org_id, std::slice::from_ref(&unknown))),
        "sharing:refused"
    );
    assert_eq!(
        code(engine.check_sharing(&unknown, &[uuid(&editable[0].id)])),
        "sharing:refused"
    );
}

#[test]
#[ignore = "exige un Vaultwarden de test"]
fn lock_purges_the_session_then_a_new_connect_works() {
    let mut engine = connected();
    assert_ne!(engine.items(None).expect("liste").len(), 0);
    engine.lock().expect("verrouillage");
    assert_eq!(code(engine.items(None)), "locked");
    assert_eq!(code(engine.folders()), "locked");
    let server = env("VAULTAGENT_TEST_SERVER");
    let email = env("VAULTAGENT_TEST_EMAIL");
    let password = Zeroizing::new(env("VAULTAGENT_TEST_PASSWORD"));
    let account = Account {
        server: &server,
        email: &email,
    };
    engine.connect(&account, &password, None).expect("reconnexion");
    assert_ne!(engine.items(None).expect("liste").len(), 0);
    engine.kill();
    assert_eq!(code(engine.items(None)), "engine-locked");
}

#[test]
#[ignore = "exige un Vaultwarden de test"]
fn bad_password_and_unreachable_server_have_their_categories() {
    let mut engine = fresh();
    let server = env("VAULTAGENT_TEST_SERVER");
    let email = env("VAULTAGENT_TEST_EMAIL");
    let account = Account {
        server: &server,
        email: &email,
    };
    let wrong = Zeroizing::new("mauvais-mot-de-passe-factice".to_owned());
    assert_eq!(code(engine.connect(&account, &wrong, None)), "login:credentials");
    assert_eq!(code(engine.items(None)), "locked");
    let nowhere = Account {
        server: "http://127.0.0.1:9",
        email: &email,
    };
    assert_eq!(code(engine.connect(&nowhere, &wrong, None)), "login:network");
}
