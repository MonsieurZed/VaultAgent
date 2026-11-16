//! Tests sans réseau du moteur natif : écritures verrouillées, état verrouillé, arrêt et purge de la session au `lock`.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::{Arc, Weak};

    use serde_json::json;
    use vaultagent_core::{NewItem, RevisionDate, Uuid};

    use crate::engine::{Engine, EngineFactory};
    use crate::passkey::Passkey;
    use crate::sdk::engine::SdkEngine;
    use crate::sdk::factory::SdkEngineFactory;
    use crate::sdk::session::Session;
    use crate::sdk::sync_handler::OrgState;

    const ID: &str = "11111111-1111-4111-8111-111111111111";

    /// Identifiant valide des tests.
    fn id() -> Uuid {
        Uuid::parse(ID).unwrap()
    }

    /// Texte du code d'échec d'un résultat.
    fn code<T>(result: vaultagent_core::Result<T>) -> Option<String> {
        result.err().map(|error| error.to_string())
    }

    /// Moteur portant une session non connectée vers une adresse locale, et un témoin de sa destruction.
    fn engine_with_session() -> (SdkEngine, Weak<OrgState>) {
        let mut engine = SdkEngine::new().unwrap();
        let session = engine.runtime.block_on(Session::build("http://127.0.0.1:9")).unwrap();
        let witness = Arc::downgrade(&session.orgs);
        engine.session = Some(session);
        (engine, witness)
    }

    #[test]
    fn a_fresh_engine_is_alive_and_reads_are_locked() {
        let mut engine = SdkEngine::new().unwrap();
        assert!(engine.alive());
        assert_eq!(code(engine.items(None)), Some("locked".into()));
        assert_eq!(code(engine.folders()), Some("locked".into()));
        assert_eq!(code(engine.sync()), Some("locked".into()));
        assert_eq!(code(engine.organizations()), Some("locked".into()));
    }

    #[test]
    fn lock_drops_the_session_and_everything_it_holds() {
        let (mut engine, witness) = engine_with_session();
        assert!(witness.upgrade().is_some());
        engine.lock().unwrap();
        assert!(witness.upgrade().is_none());
        assert_eq!(code(engine.items(Some("x"))), Some("locked".into()));
        assert_eq!(code(engine.trashed_items()), Some("locked".into()));
        assert_eq!(code(engine.item(&id())), Some("locked".into()));
        assert_eq!(code(engine.totp(&id())), Some("locked".into()));
        assert!(engine.alive());
    }

    #[test]
    fn kill_stops_the_engine_and_purges_the_session() {
        let (mut engine, witness) = engine_with_session();
        engine.kill();
        assert!(witness.upgrade().is_none() && !engine.alive());
        assert_eq!(code(engine.items(None)), Some("engine-locked".into()));
        assert_eq!(code(engine.lock()), Some("engine-locked".into()));
        engine.kill();
    }

    #[test]
    fn writes_are_locked_without_a_session_and_after_kill() {
        let mut engine = SdkEngine::new().unwrap();
        let org = Uuid::parse("22222222-2222-4222-8222-222222222222").unwrap();
        let revision = RevisionDate::parse("2026-11-16T10:00:00.000Z").unwrap();
        let item = NewItem::parse(&json!({"type": 2, "name": "n"})).unwrap();
        let passkey = Passkey {
            credential_id: "c".into(),
            key_type: "public-key".into(),
            key_algorithm: "ECDSA".into(),
            key_curve: "P-256".into(),
            key_value: crate::secret::Secret::from("fake-key"),
            rp_id: "example.invalid".into(),
            rp_name: None,
            user_handle: None,
            user_name: None,
            user_display_name: None,
            counter: "0".into(),
            discoverable: "true".into(),
            creation_date: None,
        };
        let expected = Some("locked".to_owned());
        check_writes(&mut engine, &item, &revision, &passkey, &org, expected.as_ref());
        engine.kill();
        let stopped = Some("engine-locked".to_owned());
        check_writes(&mut engine, &item, &revision, &passkey, &org, stopped.as_ref());
    }

    /// Vérifie que chaque écriture rend le même code d'échec.
    fn check_writes(
        engine: &mut SdkEngine,
        item: &NewItem,
        revision: &RevisionDate,
        passkey: &Passkey,
        org: &Uuid,
        expected: Option<&String>,
    ) {
        assert_eq!(code(engine.create_item(item)).as_ref(), expected);
        assert_eq!(
            code(engine.update_item(&id(), revision, &json!({}), None)).as_ref(),
            expected
        );
        assert_eq!(code(engine.trash_item(&id())).as_ref(), expected);
        assert_eq!(code(engine.restore_item(&id())).as_ref(), expected);
        assert_eq!(code(engine.create_collection(org, "c")).as_ref(), expected);
        assert_eq!(code(engine.share_item(&id(), org, &[id()])).as_ref(), expected);
        assert_eq!(code(engine.passkeys(None)).as_ref(), expected);
        assert_eq!(code(engine.create_passkey(passkey)).as_ref(), expected);
    }

    #[test]
    fn the_factory_starts_an_engine_without_touching_the_directory() {
        let directory = std::env::temp_dir().join("vaultagent-sdk-factory-absent");
        let mut engine = SdkEngineFactory.start(&directory, "default").unwrap();
        assert!(engine.alive());
        assert!(!directory.exists());
    }

    #[test]
    fn debug_shows_state_only() {
        let (engine, _) = engine_with_session();
        assert_eq!(format!("{engine:?}"), "SdkEngine { alive: true, connected: true, .. }");
    }
}
