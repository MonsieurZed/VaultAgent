//! Binaire `vaultagent` lancé pour de vrai : usage, aide, codes de sortie, version et refus avant ou sans réseau.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::{Command, Output};

/// Lance le binaire avec des arguments.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vaultagent"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn unknown_command_exits_one_with_a_json_error() {
    let output = run(&["steal"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout.len(), 0);
    let line: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(line["version"], 1);
    assert_eq!(line["error"], "usage");
}

#[test]
fn no_command_exits_one() {
    assert_eq!(run(&[]).status.code(), Some(1));
}

#[test]
fn help_and_version_exit_zero_on_stdout() {
    let help = run(&["--help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(String::from_utf8(help.stdout).unwrap().contains("pair-request"));
    let version = run(&["--version"]);
    assert_eq!(version.status.code(), Some(0));
    assert!(String::from_utf8(version.stdout).unwrap().contains("vaultagent"));
}

#[test]
fn insecure_server_address_is_refused_before_any_network_call() {
    let output = run(&["pair-request", "http://127.0.0.1:1", "--config", "unused.json"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout.len(), 0);
}

#[test]
fn missing_config_option_or_file_exits_one() {
    let output = run(&["status", "--config", "definitely-absent-client.json"]);
    assert_eq!(output.status.code(), Some(1));
    let line: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(line["error"], "config");
}

#[test]
fn general_help_lists_the_exit_code_table_and_every_command() {
    let help = String::from_utf8(run(&["--help"]).stdout).unwrap();
    for code in [
        "0  success",
        "1  usage",
        "2  refused",
        "3  target refused",
        "4  vault locked",
        "5  outcome unknown",
    ] {
        assert!(help.contains(code), "{code}");
    }
    for command in [
        "pair-request",
        "status",
        "vaults",
        "sharing",
        "catalog",
        "preview",
        "create",
        "update",
        "delete",
        "restore",
        "folder",
        "unlock",
        "lock",
        "seal",
    ] {
        assert!(help.contains(command), "{command}");
    }
}

#[test]
fn every_command_help_is_english_with_an_example() {
    let commands: [&[&str]; 17] = [
        &["pair-request"],
        &["status"],
        &["vaults"],
        &["sharing"],
        &["catalog"],
        &["preview"],
        &["create"],
        &["update"],
        &["delete"],
        &["restore"],
        &["folder"],
        &["unlock"],
        &["lock"],
        &["seal"],
        &["folder", "create"],
        &["folder", "rename"],
        &["folder", "delete"],
    ];
    for command in commands {
        let mut args = command.to_vec();
        args.push("--help");
        let output = run(&args);
        assert_eq!(output.status.code(), Some(0), "{command:?}");
        assert_eq!(output.stderr.len(), 0);
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Example"), "{command:?}");
        assert!(text.contains("Usage:"), "{command:?}");
        assert!(
            !text.contains("Demande") && !text.contains("Identifiant"),
            "{command:?}"
        );
    }
}

#[test]
fn seal_help_states_that_no_clear_value_is_printed() {
    let text = String::from_utf8(run(&["seal", "--help"]).stdout).unwrap();
    assert!(text.contains("--public-key"));
    assert!(text.contains("never leaves the program in clear text"));
}

#[test]
fn seal_with_a_bad_origin_exits_one_without_network() {
    let output = run(&[
        "seal",
        "0000000a-0000-4000-8000-00000000000c",
        "password",
        "http://example.test",
        "--public-key",
        "AAAA",
        "--config",
        "definitely-absent-client.json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout.len(), 0);
}

#[test]
fn unreachable_server_exits_two_with_one_error_line() {
    let dir = std::env::temp_dir().join(format!("vaultagent-bin-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("client.json");
    let token = format!("va1.0000000a-0000-4000-8000-00000000000b.{}", "A".repeat(43));
    let text = format!("{{\"server\":\"https://127.0.0.1:1\",\"token\":\"{token}\"}}");
    vaultagent_client::private_file::write_private_atomic(&config, text.as_bytes()).unwrap();
    let output = run(&["status", "--config", config.to_str().unwrap()]);
    std::fs::remove_dir_all(&dir).unwrap_or_default();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout.len(), 0);
    let line: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(line["version"], 1);
    assert_eq!(line["error"], "unreachable");
}
