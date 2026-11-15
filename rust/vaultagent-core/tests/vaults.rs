//! Registre des coffres, sélection par jeton, comptes en double et droits accordés (cahier `multivault.test.mjs`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::{Value, json};
use vaultagent_core::device_token::issue_token;
use vaultagent_core::vault_clients::{available_clients, ensure_unique_account, grant};
use vaultagent_core::vault_select::{authorized, metadata, select};
use vaultagent_core::{AccountRef, DeviceRecord, ErrorCode, SeededEntropy, VaultDevices, VaultRegistry};

const AGENT: &str = "20000000-0000-4000-8000-000000000001";

/// Registre à partir d'un JSON.
fn registry(raw: Value) -> Result<VaultRegistry, ErrorCode> {
    VaultRegistry::init(Some(&raw))
}

/// Appareil factice et son en-tête `Authorization`.
fn device(profile: &str) -> (DeviceRecord, String) {
    let issued = issue_token(&mut SeededEntropy::new(7)).unwrap();
    let record = DeviceRecord {
        id: issued.device_id.clone(),
        name: "PC factice".into(),
        profile: profile.into(),
        created: "2026-11-15T00:00:00.000Z".into(),
        hash: issued.hash.clone(),
        approver_public_key: None,
        hello_enrollment_allowed: None,
        hello_credential: None,
        elevation_public_key: None,
        auto_unlock: Some(true),
        extra: serde_json::Map::new(),
    };
    (record, format!("Bearer {}", issued.token.as_str()))
}

#[test]
fn absent_file_gives_the_default_registry() {
    let registry = VaultRegistry::init(None).unwrap();
    assert_eq!(
        registry.to_json(),
        json!([{"id": "default", "name": "Coffre existant"}])
    );
}

#[test]
fn registry_validation_matches_node() {
    let ok = registry(json!([{"id": "default", "name": "A"}, {"id": AGENT.to_uppercase(), "name": "B"}]));
    assert_eq!(ok.unwrap().entries().len(), 2);
    let bad = [
        json!({"id": "default"}),
        json!([]),
        json!([{"id": "x", "name": "A"}]),
        json!([{"id": "default", "name": "A"}, {"id": "default", "name": "B"}]),
        json!([{"id": AGENT, "name": "B"}]),
        json!(["default"]),
        json!([{"id": "default"}]),
    ];
    for raw in bad {
        assert_eq!(
            registry(raw.clone()).unwrap_err(),
            ErrorCode::InvalidVaultRegistry,
            "{raw}"
        );
    }
    let too_many: Vec<Value> = (0..9)
        .map(|n| json!({"id": if n == 0 { "default".to_owned() } else { format!("2000000{n}-0000-4000-8000-000000000001") }, "name": "N"}))
        .collect();
    assert_eq!(
        registry(Value::Array(too_many)).unwrap_err(),
        ErrorCode::InvalidVaultRegistry
    );
    assert_eq!(
        registry(json!([{"id": "default", "name": "A"}, {"id": "pas-un-uuid", "name": "B"}])).unwrap_err(),
        ErrorCode::InvalidVault
    );
}

#[test]
fn save_name_adds_and_renames_without_mutating_the_source() {
    let source = VaultRegistry::default();
    let added = source
        .save_name(None, Some("  Agent  "), &mut SeededEntropy::new(1))
        .unwrap();
    assert!(added.created);
    assert_eq!(added.entry.name, "Agent");
    assert_eq!(added.registry.entries().len(), 2);
    assert_eq!(source.entries().len(), 1);
    let fresh = vaultagent_core::Uuid::parse(&added.entry.id).unwrap();
    assert_eq!(fresh.as_str().as_bytes()[14], b'4');
    let renamed = added
        .registry
        .save_name(Some("default"), Some("Principal"), &mut SeededEntropy::new(1))
        .unwrap();
    assert!(!renamed.created);
    assert_eq!(renamed.registry.get("default").unwrap().name, "Principal");
    assert_eq!(renamed.registry.get(&added.entry.id).unwrap().name, "Agent");
}

#[test]
fn save_name_refuses_bad_names_unknown_ids_and_the_ninth_vault() {
    let mut entropy = SeededEntropy::new(2);
    let base = VaultRegistry::default();
    for name in [None, Some(""), Some("   "), Some("a\nb")] {
        assert_eq!(
            base.save_name(None, name, &mut entropy).unwrap_err(),
            ErrorCode::InvalidVaultName
        );
    }
    let long = "a".repeat(81);
    assert_eq!(
        base.save_name(None, Some(&long), &mut entropy).unwrap_err(),
        ErrorCode::InvalidVaultName
    );
    assert!(base.save_name(None, Some(&"a".repeat(80)), &mut entropy).is_ok());
    assert_eq!(
        base.save_name(Some(AGENT), Some("X"), &mut entropy).unwrap_err(),
        ErrorCode::VaultMissing
    );
    let mut full = base;
    for _ in 0..7 {
        full = full.save_name(None, Some("N"), &mut entropy).unwrap().registry;
    }
    assert_eq!(full.entries().len(), 8);
    assert_eq!(
        full.save_name(None, Some("N"), &mut entropy).unwrap_err(),
        ErrorCode::VaultLimit
    );
    assert!(full.save_name(Some("default"), Some("Renommé"), &mut entropy).is_ok());
}

#[test]
fn duplicate_account_compares_origin_and_lowercase_email() {
    let others = [AccountRef {
        server: "https://vault.example.test",
        email: "Admin@Example.test",
    }];
    let same = AccountRef {
        server: "https://VAULT.example.test:443/",
        email: "admin@example.TEST",
    };
    assert_eq!(
        ensure_unique_account(&others, &same).unwrap_err(),
        ErrorCode::DuplicateAccount
    );
    let other_email = AccountRef {
        email: "agent@example.test",
        ..same
    };
    assert!(ensure_unique_account(&others, &other_email).is_ok());
    let other_server = AccountRef {
        server: "https://autre.example.test",
        ..same
    };
    assert!(ensure_unique_account(&others, &other_server).is_ok());
}

#[test]
fn selection_follows_grants_and_refuses_ambiguity() {
    let (admin_device, header) = device("admin");
    let admin_records = [admin_device.clone()];
    let no_records: [DeviceRecord; 0] = [];
    let one = [
        VaultDevices {
            id: "default",
            profile: Some("admin"),
            records: &admin_records,
        },
        VaultDevices {
            id: AGENT,
            profile: Some("agent"),
            records: &no_records,
        },
    ];
    assert_eq!(authorized(Some(&header), &one), vec!["default"]);
    assert_eq!(select(Some(&header), None, &one).unwrap(), "default");
    assert_eq!(
        select(Some(&header), Some(AGENT), &one).unwrap_err(),
        ErrorCode::Unauthorized
    );
    assert_eq!(select(None, None, &one).unwrap_err(), ErrorCode::Unauthorized);

    let granted = grant(&one, AGENT, &admin_device.id).unwrap();
    assert!(granted.changed);
    assert_eq!(granted.records[0].profile, "agent");
    assert_eq!(granted.records[0].auto_unlock, Some(false));
    assert_eq!(admin_device.auto_unlock, Some(true));
    let two = [
        VaultDevices {
            id: "default",
            profile: Some("admin"),
            records: &admin_records,
        },
        VaultDevices {
            id: AGENT,
            profile: Some("agent"),
            records: &granted.records,
        },
    ];
    assert_eq!(select(Some(&header), None, &two).unwrap_err(), ErrorCode::VaultRequired);
    assert_eq!(select(Some(&header), Some(AGENT), &two).unwrap(), AGENT);
    assert_eq!(authorized(Some(&header), &two).len(), 2);
}

#[test]
fn grant_refusals_and_idempotence() {
    let (record, _) = device("admin");
    let records = [record.clone()];
    let other: [DeviceRecord; 0] = [];
    let vaults = [
        VaultDevices {
            id: "default",
            profile: Some("admin"),
            records: &records,
        },
        VaultDevices {
            id: AGENT,
            profile: None,
            records: &other,
        },
    ];
    assert_eq!(grant(&vaults, AGENT, &record.id).unwrap_err(), ErrorCode::NotConfigured);
    assert_eq!(
        grant(&vaults, "inconnu", &record.id).unwrap_err(),
        ErrorCode::VaultMissing
    );
    assert_eq!(
        grant(&vaults, "default", "absent").unwrap_err(),
        ErrorCode::InvalidDevice
    );
    let again = grant(&vaults, "default", &record.id).unwrap();
    assert!(!again.changed);
    assert_eq!(again.records, records.to_vec());

    let full: Vec<DeviceRecord> = (0..32)
        .map(|n| DeviceRecord {
            id: format!("id-{n}"),
            profile: "agent".into(),
            ..record.clone()
        })
        .collect();
    let crowded = [
        VaultDevices {
            id: "default",
            profile: Some("admin"),
            records: &records,
        },
        VaultDevices {
            id: AGENT,
            profile: Some("agent"),
            records: &full,
        },
    ];
    assert_eq!(grant(&crowded, AGENT, &record.id).unwrap_err(), ErrorCode::DeviceLimit);
}

#[test]
fn available_clients_lists_each_device_once() {
    let (record, _) = device("admin");
    let copy = DeviceRecord {
        profile: "agent".into(),
        ..record.clone()
    };
    let (a, b) = ([record.clone()], [copy]);
    let vaults = [
        VaultDevices {
            id: "default",
            profile: Some("admin"),
            records: &a,
        },
        VaultDevices {
            id: AGENT,
            profile: Some("agent"),
            records: &b,
        },
    ];
    let clients = available_clients(&vaults);
    assert_eq!(clients.len(), 1);
    assert_eq!(
        (clients[0].id.as_str(), clients[0].name.as_str()),
        (record.id.as_str(), "PC factice")
    );
}

#[test]
fn golden_vaults_case_is_rebuilt_from_metadata() {
    let case: Value = serde_json::from_str(include_str!("../../contrat/vaults.json")).unwrap();
    let expected = &case["response"]["body"][0];
    let status = json!({
        "version": expected["version"], "configured": true, "unlocked": true,
        "timeoutMinutes": 15, "expiresUtc": expected["expiresUtc"]
    });
    let rows = metadata(vec![("default".to_owned(), "Coffre".to_owned(), status)]);
    assert_eq!(serde_json::to_value(&rows).unwrap()[0], *expected);
}
