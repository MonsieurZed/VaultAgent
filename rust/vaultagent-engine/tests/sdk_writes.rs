//! Intégration des écritures du moteur natif contre un Vaultwarden jetable : création, mise à jour avec contrôle de
//! révision, corbeille, restauration, collection, partage et passkeys. Tous les tests sont `#[ignore]` et lisent
//! `VAULTAGENT_TEST_SERVER`, `VAULTAGENT_TEST_EMAIL` et `VAULTAGENT_TEST_PASSWORD` (compte factice, organisation
//! « Org factice » existante). Chaque test met à la corbeille les fiches qu'il crée.
//!
//! Lancement : `cargo test -p vaultagent-engine --test sdk_writes -- --ignored`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use vaultagent_core::{NewItem, RevisionDate, Uuid};
use vaultagent_engine::{Account, Engine, EngineFactory, Passkey, SdkEngineFactory, Secret};
use zeroize::Zeroizing;

/// Valeur d'une variable d'environnement obligatoire.
fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("variable {name} absente"))
}

/// Moteur connecté au compte factice de la configuration.
fn connected() -> Box<dyn Engine> {
    let mut engine = SdkEngineFactory.start(Path::new("absent"), "test").expect("démarrage");
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

/// Crée une fiche de connexion factice et rend son identifiant.
fn create_login(engine: &mut dyn Engine, name: &str) -> String {
    let item = NewItem::parse(&json!({
        "type": 1, "name": name, "notes": "note factice",
        "login": {"username": "alice", "password": "fictif-1", "uris": [{"uri": "https://a.example.invalid"}]}
    }))
    .expect("fiche valide");
    engine.create_item(&item).expect("création")
}

#[test]
#[ignore = "exige un Vaultwarden de test"]
fn create_read_update_and_stale_revision_conflict() {
    let mut engine = connected();
    let name = format!("Test 2.1d {}", unique());
    let id = create_login(engine.as_mut(), &name);
    let created = engine.item(&uuid(&id)).expect("lecture");
    assert_eq!((created.name.as_str(), created.kind), (name.as_str(), 1));
    assert_eq!(
        created.login.as_ref().and_then(|l| l.username.as_deref()),
        Some("alice")
    );
    let first = RevisionDate::parse(&created.revision_date).expect("révision");

    let renamed = format!("{name} renommée");
    let patch = json!({"name": renamed, "notes": "nouvelle note", "addUris": ["https://b.example.invalid"]});
    engine
        .update_item(&uuid(&id), &first, &patch, None)
        .expect("mise à jour");
    let updated = engine.item(&uuid(&id)).expect("relecture");
    assert_eq!(updated.name, renamed);
    assert_eq!(updated.login.as_ref().map(|l| l.uris.len()), Some(2));
    assert_eq!(
        updated.login.as_ref().and_then(|l| l.username.as_deref()),
        Some("alice")
    );
    assert_ne!(updated.revision_date, created.revision_date);

    assert_eq!(
        code(engine.update_item(&uuid(&id), &first, &json!({"name": "x"}), None)),
        "revision-conflict"
    );
    let current = RevisionDate::parse(&updated.revision_date).expect("révision");
    let other_org = uuid("99999999-9999-4999-8999-999999999999");
    assert_eq!(
        code(engine.update_item(&uuid(&id), &current, &json!({"name": "x"}), Some(&other_org))),
        "item-refused"
    );
    assert_eq!(
        code(engine.update_item(&uuid(&id), &current, &json!({"type": 2}), None)),
        "invalid-patch"
    );
    assert_eq!(engine.item(&uuid(&id)).expect("inchangée").name, renamed);
    engine.trash_item(&uuid(&id)).expect("nettoyage");
}

#[test]
#[ignore = "exige un Vaultwarden de test"]
fn trash_then_restore() {
    let mut engine = connected();
    let id = create_login(engine.as_mut(), &format!("Test 2.1d corbeille {}", unique()));
    engine.trash_item(&uuid(&id)).expect("corbeille");
    assert!(engine.trashed_items().expect("corbeille").iter().any(|i| i.id == id));
    assert!(!engine.items(None).expect("liste").iter().any(|i| i.id == id));
    engine.sync().expect("resynchronisation");
    let trashed = engine.item(&uuid(&id)).expect("lecture");
    let stale = RevisionDate::parse(&trashed.revision_date).expect("révision");
    assert_eq!(
        code(engine.update_item(&uuid(&id), &stale, &json!({"name": "x"}), None)),
        "item-refused"
    );
    engine.restore_item(&uuid(&id)).expect("restauration");
    assert!(engine.items(None).expect("liste").iter().any(|i| i.id == id));
    assert!(!engine.trashed_items().expect("corbeille").iter().any(|i| i.id == id));
    engine.trash_item(&uuid(&id)).expect("nettoyage");
}

#[test]
#[ignore = "exige un Vaultwarden de test"]
fn collection_creation_and_item_sharing() {
    let mut engine = connected();
    let org = engine
        .organizations()
        .expect("organisations")
        .into_iter()
        .find(|o| o.name == "Org factice")
        .expect("une organisation factice");
    let org_id = uuid(&org.id);
    let collection_name = format!("Collection 2.1d {}", unique());
    let collection = engine.create_collection(&org_id, &collection_name).expect("collection");
    let listed = engine.organizations().expect("organisations");
    let found = listed.iter().find(|o| o.id == org.id).expect("organisation");
    assert!(
        found
            .collections
            .iter()
            .any(|c| c.id == collection && c.name == collection_name && c.can_edit)
    );

    let id = create_login(engine.as_mut(), &format!("Test 2.1d partage {}", unique()));
    engine
        .share_item(&uuid(&id), &org_id, &[uuid(&collection)])
        .expect("partage");
    let shared = engine.item(&uuid(&id)).expect("relecture");
    assert_eq!(shared.organization_id.as_deref(), Some(org.id.as_str()));
    assert!(shared.collection_ids.contains(&collection));
    assert_eq!(shared.login.as_ref().and_then(|l| l.username.as_deref()), Some("alice"));

    let revision = RevisionDate::parse(&shared.revision_date).expect("révision");
    engine
        .update_item(&uuid(&id), &revision, &json!({"notes": "note partagée"}), Some(&org_id))
        .expect("mise à jour d'une fiche d'organisation");
    assert_eq!(
        code(engine.share_item(&uuid(&id), &org_id, &[uuid(&collection)])),
        "sharing:refused"
    );
    let unknown = uuid("99999999-9999-4999-8999-999999999999");
    assert_eq!(
        code(engine.share_item(&uuid(&id), &org_id, &[unknown])),
        "sharing:refused"
    );
    engine.trash_item(&uuid(&id)).expect("nettoyage");
}

#[test]
#[ignore = "exige un Vaultwarden de test"]
fn passkey_creation_then_listing() {
    let mut engine = connected();
    let tag = unique();
    let rp_id = format!("rp-{tag}.example.invalid");
    let passkey = Passkey {
        credential_id: format!("credential-{tag}"),
        key_type: "public-key".into(),
        key_algorithm: "ECDSA".into(),
        key_curve: "P-256".into(),
        key_value: Secret::from("fake-passkey-key-value"),
        rp_id: rp_id.clone(),
        rp_name: Some("Exemple".into()),
        user_handle: Some("dXNlci1mYWN0aWNl".into()),
        user_name: Some("alice".into()),
        user_display_name: Some("Alice".into()),
        counter: "0".into(),
        discoverable: "true".into(),
        creation_date: None,
    };
    let id = engine.create_passkey(&passkey).expect("création de la passkey");
    let listed = engine.passkeys(Some(&rp_id)).expect("liste");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].item_id, id);
    assert_eq!(listed[0].credential.credential_id, passkey.credential_id);
    assert_eq!(listed[0].credential.key_value.expose(), "fake-passkey-key-value");
    assert_eq!(listed[0].credential.user_handle, passkey.user_handle);
    assert!(engine.passkeys(None).expect("liste").iter().any(|e| e.item_id == id));
    assert_eq!(engine.passkeys(Some("absent.example.invalid")).expect("liste").len(), 0);
    engine.trash_item(&uuid(&id)).expect("nettoyage");
    assert_eq!(engine.passkeys(Some(&rp_id)).expect("liste").len(), 0);
}
