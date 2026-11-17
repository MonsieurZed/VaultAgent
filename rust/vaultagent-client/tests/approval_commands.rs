//! Commandes `approvals`, `decide`, `elevation-enroll` et `elevation-sign` de bout en bout contre un faux serveur et
//! un faux système d'élévation : aucune invite UAC, aucun dossier réel.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::rc::Rc;

use common::{TempDir, execute, execute_elevated};
use vaultagent_client::elevation_sign::run_elevation_sign;
use vaultagent_client::exit_code::ExitCode;
use vaultagent_client::fake_elevation::FakeElevation;
use vaultagent_client::fake_transport::FakeTransport;

const SID: &str = "S-1-5-21-1-2-3-1001";

const STATUS: &str = "{\"approvalRequests\":[{\"vaultId\":\"default\",\"requestId\":\"r1\",\"challenge\":\"chal\",\
\"deviceName\":\"Box\",\"helloRequired\":false,\"elevationRequired\":false}]}";

#[test]
fn approvals_lists_the_pending_requests_without_the_device_secret() {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(FakeTransport::new().reply(200, STATUS));
    let outcome = execute(&fake, &["approvals", "--config", config.to_str().unwrap()]);
    assert_eq!(outcome.exit, ExitCode::Success);
    let value: serde_json::Value = serde_json::from_str(outcome.out.trim()).unwrap();
    assert_eq!(value["approvalRequests"][0]["requestId"], "r1");
    assert_eq!(value["approvalRequests"][0]["elevationRequired"], false);
    assert!(!outcome.out.contains("deviceName"));
}

#[test]
fn decide_with_plain_config_is_a_usage_error_and_sends_nothing_signed() {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(FakeTransport::new().reply(200, STATUS));
    let outcome = execute(
        &fake,
        &["decide", "r1", "--approve", "--config", config.to_str().unwrap()],
    );
    assert_eq!(outcome.exit, ExitCode::Usage);
    assert!(outcome.err.contains("protected store"));
    assert!(fake.calls().iter().all(|call| !call.path.ends_with("decide-unlock")));
}

#[test]
fn elevation_sign_without_elevation_exits_two_with_elevation_required() {
    let dir = TempDir::new();
    let output = dir.path().join("out.txt");
    let fake = Rc::new(FakeTransport::new());
    let system = FakeElevation::standard(SID, Box::new(|_| Ok(0)));
    let args = ["elevation-sign", SID, "bQ==", output.to_str().unwrap()];
    let outcome = execute_elevated(&fake, &args, b"", (None, Some(dir.path())), &system);
    assert_eq!(outcome.exit.code(), 2);
    let line: serde_json::Value = serde_json::from_str(outcome.err.trim()).unwrap();
    assert_eq!(line["error"], "elevation-required");
    assert!(outcome.out.is_empty() && !output.exists());
}

#[test]
fn elevation_sign_elevated_writes_two_lines_and_prints_nothing() {
    let dir = TempDir::new();
    let output = dir.path().join("out.txt");
    let fake = Rc::new(FakeTransport::new());
    let system = FakeElevation::elevated(SID);
    let args = ["elevation-sign", SID, "bQ==", output.to_str().unwrap()];
    let outcome = execute_elevated(&fake, &args, b"", (None, Some(dir.path())), &system);
    assert_eq!(outcome.exit, ExitCode::Success);
    assert_eq!(outcome.out.len(), 0);
    assert_eq!(std::fs::read_to_string(&output).unwrap().lines().count(), 2);
}

#[cfg(windows)]
mod protected {
    use super::*;
    use vaultagent_client::approver_key::ApproverKey;
    use vaultagent_client::config::ClientConfig;
    use vaultagent_client::config_store::ConfigStore;
    use vaultagent_client::server_address::ServerAddress;
    use vaultagent_client::token::Token;
    use vaultagent_core::{ErrorCode, decision_message, verify_pss};

    const DECIDED: &str = "{\"vaultId\":\"default\",\"requestId\":\"r1\",\"state\":\"approved\"}";

    /// Écrit une configuration protégée de test et rend le dossier et la clé d'approbateur.
    fn prepare(dir: &TempDir) -> (std::path::PathBuf, ApproverKey) {
        let data = dir.path().join("VaultAgent");
        let store = ConfigStore::Protected(data.clone());
        store
            .save(&ClientConfig {
                server: ServerAddress::parse("https://vault.example.test").unwrap(),
                token: Token::parse(&common::token()).unwrap(),
            })
            .unwrap();
        let key = store.approver_key().unwrap();
        (data, key)
    }

    /// Réponse `status` avec ou sans exigence d'élévation.
    fn requirement(elevation: bool) -> String {
        STATUS.replace(
            "\"elevationRequired\":false",
            &format!("\"elevationRequired\":{elevation}"),
        )
    }

    #[test]
    fn approve_without_requirement_signs_and_sends_one_decision() {
        let dir = TempDir::new();
        let (data, key) = prepare(&dir);
        let fake = Rc::new(FakeTransport::new().reply(200, &requirement(false)).reply(200, DECIDED));
        let system = FakeElevation::standard(SID, Box::new(|_| Ok(1)));
        let outcome = execute_elevated(&fake, &["decide", "r1", "--approve"], b"", (Some(&data), None), &system);
        assert_eq!(outcome.exit, ExitCode::Success, "{}", outcome.err);
        assert_eq!(outcome.out.trim(), DECIDED);
        assert_eq!(system.launches().len(), 0);
        let body: serde_json::Value = serde_json::from_str(&fake.calls()[1].body).unwrap();
        let message = decision_message("https://vault.example.test", "default", "r1", "chal", true, false);
        let public = key.public_key().unwrap();
        verify_pss(
            &public,
            &message,
            body["signature"].as_str().unwrap(),
            ErrorCode::InvalidKey,
        )
        .unwrap();
        assert!(body.get("elevationSignature").is_none());
    }

    #[test]
    fn denial_never_launches_the_elevated_process() {
        let dir = TempDir::new();
        let (data, _key) = prepare(&dir);
        let fake = Rc::new(FakeTransport::new().reply(200, &requirement(true)).reply(200, DECIDED));
        let system = FakeElevation::standard(SID, Box::new(|_| Ok(1)));
        let outcome = execute_elevated(&fake, &["decide", "r1", "--deny"], b"", (Some(&data), None), &system);
        assert_eq!(outcome.exit, ExitCode::Success, "{}", outcome.err);
        assert_eq!(system.launches().len(), 0);
        let body: serde_json::Value = serde_json::from_str(&fake.calls()[1].body).unwrap();
        assert_eq!(body["approve"], false);
    }

    #[test]
    fn approve_with_uac_adds_the_elevation_signature_from_a_simulated_child() {
        let dir = TempDir::new();
        let (data, _key) = prepare(&dir);
        let program_data = TempDir::new();
        let program_path = program_data.path().to_path_buf();
        let fake = Rc::new(FakeTransport::new().reply(200, &requirement(true)).reply(200, DECIDED));
        let system = FakeElevation::standard(
            SID,
            Box::new(move |args| {
                let parts: Vec<&str> = args.splitn(4, ' ').collect();
                let output = std::path::PathBuf::from(parts[3].trim_matches('"'));
                run_elevation_sign(
                    &FakeElevation::elevated(SID),
                    Some(&program_path),
                    parts[1],
                    parts[2],
                    &output,
                )
                .map(|()| 0)
            }),
        );
        let outcome = execute_elevated(&fake, &["decide", "r1", "--approve"], b"", (Some(&data), None), &system);
        assert_eq!(outcome.exit, ExitCode::Success, "{}", outcome.err);
        let body: serde_json::Value = serde_json::from_str(&fake.calls()[1].body).unwrap();
        assert!(body["elevationSignature"].as_str().unwrap().len() > 100);
        assert_eq!(system.launches().len(), 1);
    }

    #[test]
    fn cancelled_uac_sends_no_decision_and_exits_two() {
        let dir = TempDir::new();
        let (data, _key) = prepare(&dir);
        let fake = Rc::new(FakeTransport::new().reply(200, &requirement(true)));
        let system = FakeElevation::standard(
            SID,
            Box::new(|_| Err(vaultagent_client::error::ClientError::ElevationCancelled)),
        );
        let outcome = execute_elevated(&fake, &["decide", "r1", "--approve"], b"", (Some(&data), None), &system);
        assert_eq!(outcome.exit.code(), 2);
        assert!(outcome.err.contains("elevation-cancelled"));
        assert_eq!(fake.calls().len(), 1);
    }

    #[test]
    fn unknown_request_is_refused_without_a_decision() {
        let dir = TempDir::new();
        let (data, _key) = prepare(&dir);
        let fake = Rc::new(FakeTransport::new().reply(200, &requirement(false)));
        let system = FakeElevation::standard(SID, Box::new(|_| Ok(1)));
        let outcome = execute_elevated(&fake, &["decide", "nope", "--deny"], b"", (Some(&data), None), &system);
        assert_eq!(outcome.exit.code(), 2);
        assert!(outcome.err.contains("request-not-found"));
        assert_eq!(fake.calls().len(), 1);
    }

    #[test]
    fn elevation_enroll_prints_the_count() {
        let dir = TempDir::new();
        let (data, _key) = prepare(&dir);
        let program_data = TempDir::new();
        let program_path = program_data.path().to_path_buf();
        let fake = Rc::new(
            FakeTransport::new()
                .reply(200, "[{\"id\":\"default\"}]")
                .reply(
                    200,
                    "{\"approvalSecurity\":{\"registered\":true,\"elevationEnabled\":false}}",
                )
                .reply(
                    200,
                    "{\"vaultId\":\"default\",\"requestId\":\"enroll-elevation\",\"challenge\":\"chal\"}",
                )
                .reply(200, "{\"elevationEnabled\":true,\"vaultId\":\"default\"}"),
        );
        let system = FakeElevation::standard(
            SID,
            Box::new(move |args| {
                let parts: Vec<&str> = args.splitn(4, ' ').collect();
                let output = std::path::PathBuf::from(parts[3].trim_matches('"'));
                run_elevation_sign(
                    &FakeElevation::elevated(SID),
                    Some(&program_path),
                    parts[1],
                    parts[2],
                    &output,
                )
                .map(|()| 0)
            }),
        );
        let outcome = execute_elevated(&fake, &["elevation-enroll"], b"", (Some(&data), None), &system);
        assert_eq!(outcome.exit, ExitCode::Success, "{}", outcome.err);
        assert_eq!(outcome.out.trim(), "{\"enrolled\":1}");
        let enroll: serde_json::Value = serde_json::from_str(&fake.calls()[3].body).unwrap();
        assert_eq!(enroll["vaultId"], "default");
        assert!(enroll["publicKey"].as_str().unwrap().len() > 100);
    }
}

#[test]
fn hello_status_prints_the_system_state_without_any_request() {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(FakeTransport::new());
    let outcome = execute(&fake, &["hello-status", "--config", config.to_str().unwrap()]);
    assert_eq!(outcome.exit, ExitCode::Success);
    assert_eq!(outcome.out.trim(), "{\"available\":false,\"apiVersion\":0}");
    assert_eq!(fake.calls().len(), 0);
}

#[test]
fn hello_enroll_without_hello_is_refused_before_any_request() {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(FakeTransport::new());
    let outcome = execute(&fake, &["hello-enroll", "--config", config.to_str().unwrap()]);
    assert_eq!(outcome.exit, ExitCode::Usage);
    assert!(outcome.err.contains("protected store"));
    assert_eq!(fake.calls().len(), 0);
}
