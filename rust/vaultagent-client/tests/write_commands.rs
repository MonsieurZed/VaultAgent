//! Commandes d'écriture de bout en bout contre un faux serveur : corps de la forme Node, refus avant envoi, secret absent.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::rc::Rc;

use common::{Outcome, TempDir, execute_with_input};
use serde_json::Value;
use vaultagent_client::fake_transport::FakeTransport;
use vaultagent_client::transport::TransportError;

const OP: &str = "0000000a-0000-4000-8000-00000000000d";
const ITEM: &str = "0000000a-0000-4000-8000-00000000000c";
const FOLDER: &str = "0000000a-0000-4000-8000-00000000000e";
const ORG: &str = "0000000a-0000-4000-8000-00000000000f";
const DATE: &str = "2026-11-08T00:00:00.000Z";
const SENTINEL: &str = "SENTINEL-do-not-print-42";
const RECEIPT: &str = "{\"id\":\"x\",\"verified\":true}";

/// Exécute une commande avec une configuration valide et rend le résultat et le faux serveur.
fn run(fake: FakeTransport, args: &[&str], input: &[u8]) -> (Outcome, Rc<FakeTransport>) {
    let dir = TempDir::new();
    let config = dir.write_config();
    let fake = Rc::new(fake);
    let mut all: Vec<&str> = args.to_vec();
    all.extend(["--config", config.to_str().unwrap()]);
    let outcome = execute_with_input(&fake, &all, input);
    (outcome, fake)
}

/// Corps JSON de la première requête.
fn sent(fake: &FakeTransport) -> Value {
    serde_json::from_str(&fake.calls()[0].body).unwrap()
}

/// Analyse un JSON attendu.
fn expected(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn create_login_sends_the_node_body_and_relays_the_receipt() {
    let metadata = "{\"type\":1,\"name\":\"Site\",\"login\":{\"uris\":[{\"uri\":\"https://example.test\"}]}}";
    let secrets = format!("{{\"username\":\"u\",\"password\":\"{SENTINEL}\",\"totp\":\"SEED\",\"notes\":\"n\"}}");
    let (outcome, fake) = run(
        FakeTransport::new().reply(200, RECEIPT),
        &["create", "--operation-id", OP, "--metadata", metadata, "--values-stdin"],
        secrets.as_bytes(),
    );
    assert_eq!(outcome.exit.code(), 0);
    assert_eq!(outcome.out, format!("{RECEIPT}\n"));
    assert_eq!(fake.calls()[0].path, "/api/agent/write");
    let body = format!(
        "{{\"operationId\":\"{OP}\",\"action\":\"create\",\"item\":{{\"type\":1,\"name\":\"Site\",\"notes\":\"n\",\
         \"login\":{{\"uris\":[{{\"uri\":\"https://example.test\"}}],\"username\":\"u\",\"password\":\"{SENTINEL}\",\
         \"totp\":\"SEED\"}}}}}}"
    );
    assert_eq!(sent(&fake), expected(&body));
    assert!(!outcome.out.contains(SENTINEL) && !outcome.err.contains(SENTINEL));
}

#[test]
fn create_note_and_login_without_values_follow_node() {
    let (outcome, fake) = run(
        FakeTransport::new().reply(200, RECEIPT),
        &[
            "create",
            "--operation-id",
            OP,
            "--metadata",
            "{\"type\":2,\"name\":\"N\"}",
            "--values-stdin",
        ],
        b"{\"notes\":\"secret note\"}",
    );
    assert_eq!(outcome.exit.code(), 0);
    let body = format!(
        "{{\"operationId\":\"{OP}\",\"action\":\"create\",\"item\":{{\"type\":2,\"name\":\"N\",\"notes\":\"secret note\"}}}}"
    );
    assert_eq!(sent(&fake), expected(&body));

    let (_, fake) = run(
        FakeTransport::new().reply(200, RECEIPT),
        &[
            "--vault",
            "work",
            "create",
            "--operation-id",
            OP,
            "--metadata",
            "{\"type\":1,\"name\":\"L\"}",
        ],
        b"",
    );
    let body = format!(
        "{{\"vaultId\":\"work\",\"operationId\":\"{OP}\",\"action\":\"create\",\"item\":{{\"type\":1,\"name\":\"L\",\"login\":{{}}}}}}"
    );
    assert_eq!(sent(&fake), expected(&body));
}

#[test]
fn update_sends_a_patch_with_metadata_and_values() {
    let (outcome, fake) = run(
        FakeTransport::new().reply(200, RECEIPT),
        &[
            "update",
            ITEM,
            "--operation-id",
            OP,
            "--expected-revision",
            DATE,
            "--organization-id",
            ORG,
            "--metadata",
            "{\"name\":\"Renamed\",\"addUris\":[\"https://a.test\"]}",
            "--values-stdin",
        ],
        format!("{{\"password\":\"{SENTINEL}\"}}").as_bytes(),
    );
    assert_eq!(outcome.exit.code(), 0);
    let body = format!(
        "{{\"operationId\":\"{OP}\",\"action\":\"update\",\"itemId\":\"{ITEM}\",\"expectedRevisionDate\":\"{DATE}\",\
         \"organizationId\":\"{ORG}\",\"patch\":{{\"name\":\"Renamed\",\"addUris\":[\"https://a.test\"],\"password\":\"{SENTINEL}\"}}}}"
    );
    assert_eq!(sent(&fake), expected(&body));
    assert!(!outcome.out.contains(SENTINEL) && !outcome.err.contains(SENTINEL));
}

#[test]
fn delete_and_restore_send_their_actions() {
    for action in ["delete", "restore"] {
        let (outcome, fake) = run(
            FakeTransport::new().reply(200, RECEIPT),
            &[action, ITEM, "--operation-id", OP, "--expected-revision", DATE],
            b"",
        );
        assert_eq!(outcome.exit.code(), 0);
        let body = format!(
            "{{\"operationId\":\"{OP}\",\"action\":\"{action}\",\"itemId\":\"{ITEM}\",\"expectedRevisionDate\":\"{DATE}\"}}"
        );
        assert_eq!(sent(&fake), expected(&body));
    }
}

#[test]
fn folder_commands_send_their_bodies() {
    let cases: [(Vec<&str>, String); 3] = [
        (
            vec!["folder", "create", "Dossier", "--operation-id", OP],
            format!("{{\"operationId\":\"{OP}\",\"action\":\"create-folder\",\"item\":{{\"name\":\"Dossier\"}}}}"),
        ),
        (
            vec!["folder", "rename", FOLDER, "Neuf", "--operation-id", OP],
            format!(
                "{{\"operationId\":\"{OP}\",\"action\":\"update-folder\",\"folderId\":\"{FOLDER}\",\"item\":{{\"name\":\"Neuf\"}}}}"
            ),
        ),
        (
            vec!["folder", "delete", FOLDER, "--operation-id", OP],
            format!("{{\"operationId\":\"{OP}\",\"action\":\"delete-folder\",\"folderId\":\"{FOLDER}\"}}"),
        ),
    ];
    for (args, body) in cases {
        let (outcome, fake) = run(FakeTransport::new().reply(200, RECEIPT), &args, b"");
        assert_eq!(outcome.exit.code(), 0, "{args:?}");
        assert_eq!(sent(&fake), expected(&body), "{args:?}");
    }
}

#[test]
fn secrets_in_metadata_or_bad_input_are_refused_before_sending() {
    let leak = format!("{{\"type\":1,\"name\":\"x\",\"password\":\"{SENTINEL}\"}}");
    let nested = format!("{{\"type\":1,\"login\":{{\"password\":\"{SENTINEL}\"}}}}");
    let bad_stdin = format!("{{\"token\":\"{SENTINEL}\"}}");
    let big = format!("{{\"notes\":\"{}\"}}", "a".repeat(70_000));
    let cases: [(Vec<&str>, Vec<u8>); 7] = [
        (vec!["create", "--operation-id", OP, "--metadata", &leak], vec![]),
        (vec!["create", "--operation-id", OP, "--metadata", &nested], vec![]),
        (
            vec![
                "create",
                "--operation-id",
                OP,
                "--metadata",
                "{\"type\":1}",
                "--values-stdin",
            ],
            bad_stdin.into_bytes(),
        ),
        (
            vec![
                "create",
                "--operation-id",
                OP,
                "--metadata",
                "{\"type\":1}",
                "--values-stdin",
            ],
            b"not json".to_vec(),
        ),
        (
            vec![
                "create",
                "--operation-id",
                OP,
                "--metadata",
                "{\"type\":1}",
                "--values-stdin",
            ],
            big.into_bytes(),
        ),
        (
            vec![
                "create",
                "--operation-id",
                OP,
                "--metadata",
                "{\"type\":2}",
                "--values-stdin",
            ],
            format!("{{\"password\":\"{SENTINEL}\"}}").into_bytes(),
        ),
        (
            vec!["create", "--operation-id", "not-a-uuid", "--metadata", "{}"],
            vec![],
        ),
    ];
    for (args, input) in cases {
        let (outcome, fake) = run(FakeTransport::new(), &args, &input);
        assert_eq!(outcome.exit.code(), 1, "{args:?}");
        assert_eq!(fake.calls().len(), 0, "{args:?}");
        assert!(
            !outcome.out.contains(SENTINEL) && !outcome.err.contains(SENTINEL),
            "{args:?}"
        );
    }
}

#[test]
fn invalid_targets_never_reach_the_network() {
    let cases: [Vec<&str>; 4] = [
        vec!["delete", "nope", "--operation-id", OP, "--expected-revision", DATE],
        vec!["restore", ITEM, "--operation-id", OP, "--expected-revision", ""],
        vec!["folder", "delete", "nope", "--operation-id", OP],
        vec!["update", ITEM, "--operation-id", OP, "--expected-revision", DATE],
    ];
    for args in cases {
        let (outcome, fake) = run(FakeTransport::new(), &args, b"");
        assert_eq!(outcome.exit.code(), 1, "{args:?}");
        assert_eq!(fake.calls().len(), 0, "{args:?}");
    }
}

#[test]
fn indeterminate_result_exits_five_and_keeps_the_advice_without_values() {
    for fake in [
        FakeTransport::new().fail(TransportError::Indeterminate),
        FakeTransport::new().reply(500, "{\"error\":\"write-indeterminate\"}"),
    ] {
        let (outcome, _) = run(
            fake,
            &[
                "create",
                "--operation-id",
                OP,
                "--metadata",
                "{\"type\":1}",
                "--values-stdin",
            ],
            format!("{{\"password\":\"{SENTINEL}\"}}").as_bytes(),
        );
        assert_eq!(outcome.exit.code(), 5);
        assert!(outcome.err.contains("operationId") && outcome.err.contains("preview"));
        assert!(!outcome.out.contains(SENTINEL) && !outcome.err.contains(SENTINEL));
    }
}

#[test]
fn server_refusals_keep_their_codes() {
    let (locked, _) = run(
        FakeTransport::new().reply(423, "{\"error\":\"locked\"}"),
        &["folder", "create", "D", "--operation-id", OP],
        b"",
    );
    assert_eq!(locked.exit.code(), 4);
    let (conflict, _) = run(
        FakeTransport::new().reply(409, "{\"error\":\"revision-conflict\"}"),
        &["delete", ITEM, "--operation-id", OP, "--expected-revision", DATE],
        b"",
    );
    assert_eq!(conflict.exit.code(), 2);
    assert!(conflict.err.contains("revision-conflict"));
}
