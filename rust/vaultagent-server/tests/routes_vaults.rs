//! Plusieurs coffres : autorisation par coffre, sélection explicite, acceptation automatique d'un déverrouillage.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::service::{Client, Service, VaultState};
use serde_json::json;
use vaultagent_server::ConfigureInput;
use zeroize::Zeroizing;

/// Aide de test : with second vault.
fn with_second_vault(authorize_second: bool) -> (Service, String) {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let second = service.vaults.save_name(None, Some("Second")).unwrap().id;
    let context = service.vaults.get(Some(&second)).unwrap();
    service.factory.engine().push_ok();
    let input = ConfigureInput {
        server: common::SERVER.to_owned(),
        email: "autre@example.invalid".to_owned(),
        password: Some(Zeroizing::new(common::FICTITIOUS.to_owned())),
        timeout_minutes: Some(json!(15)),
        code: None,
    };
    service.vaults.configure(&context, &input).unwrap();
    if authorize_second {
        let mut record = service.vaults.get(None).unwrap().store.devices()[0].clone();
        record.profile = context.store.account().unwrap().profile;
        context.store.update_devices(|_| Ok(vec![record])).unwrap();
    }
    (service, second)
}

#[test]
fn a_token_only_reaches_the_vaults_it_was_granted() {
    let (service, second) = with_second_vault(false);
    let (status, list) = service.call("vaults", &json!({}));
    assert_eq!(status, 200);
    assert_eq!(list.as_array().map(Vec::len), Some(1));
    assert_eq!(list[0]["id"], "default");
    assert_eq!(
        service.call("status", &json!({"vaultId": second})),
        (403, json!({"error": "unauthorized"}))
    );
    assert_eq!(service.call("status", &json!({})).1["vaultId"], "default");
}

#[test]
fn several_authorized_vaults_require_an_explicit_choice() {
    let (service, second) = with_second_vault(true);
    let (_, list) = service.call("vaults", &json!({}));
    assert_eq!(list.as_array().map(Vec::len), Some(2));
    assert_eq!(
        service.call("status", &json!({})),
        (400, json!({"error": "vault-required"}))
    );
    assert_eq!(
        service.call("catalog", &json!({})),
        (400, json!({"error": "vault-required"}))
    );
    let (status, chosen) = service.call("status", &json!({"vaultId": second}));
    assert_eq!((status, chosen["vaultName"].clone()), (200, json!("Second")));
    assert_eq!(
        service.call("lock", &json!({"vaultId": second})).1,
        json!({"locked": true})
    );
    assert_eq!(service.call("status", &json!({"vaultId": second})).1["unlocked"], false);
    assert_eq!(
        service.call("status", &json!({"vaultId": "default"})).1["unlocked"],
        true
    );
}

#[test]
fn an_automatic_device_opens_the_vault_on_request() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let vault = service.vaults.get(None).unwrap();
    vault
        .store
        .update_devices(|records| {
            let mut next = records.to_vec();
            next[0].auto_unlock = Some(true);
            Ok(next)
        })
        .unwrap();
    service.factory.engine().push_ok();
    let (status, answer) = service.call("request-unlock", &json!({}));
    assert_eq!(
        (status, answer["state"].clone(), answer["unlocked"].clone()),
        (200, json!("approved"), json!(true))
    );
    assert_eq!(service.call("status", &json!({})).1["unlocked"], true);
}

#[test]
fn revoking_the_device_refuses_further_requests() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    service
        .vaults
        .get(None)
        .unwrap()
        .store
        .update_devices(|_| Ok(Vec::new()))
        .unwrap();
    assert_eq!(
        service.call("status", &json!({})),
        (403, json!({"error": "unauthorized"}))
    );
    assert_eq!(
        service.call("vaults", &json!({})),
        (403, json!({"error": "unauthorized"}))
    );
}
