//! Téléchargements et fichiers du front servis au propriétaire.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TempDir;
use common::service::{Client, OWNER, Service, VaultState};
use vaultagent_server::{Context, Request};

const ARCHIVE: &str = "VaultAgent-Linux-Client-3.0.0-preview.7.tar.gz";

/// Requête `GET` du propriétaire.
fn get(path: &str) -> Request {
    Request::new("GET", path, "127.0.0.1").with_header("x-authentik-username", OWNER)
}

/// Service dont le dossier des téléchargements est celui donné.
fn with_downloads(downloads: Option<&TempDir>) -> (Service, Context) {
    let service = Service::new(VaultState::Locked, Client::None);
    let context = Context {
        downloads_directory: downloads.map(|dir| dir.path().to_path_buf()),
        ..service.context.clone()
    };
    (service, context)
}

#[test]
fn an_archive_is_sent_as_an_attachment() {
    let downloads = TempDir::new("downloads");
    std::fs::write(downloads.path().join(ARCHIVE), b"archive factice").unwrap();
    let (_service, context) = with_downloads(Some(&downloads));
    let response = vaultagent_server::route(&get("/downloads/linux-client.tar.gz"), &context);
    assert_eq!(
        (response.status, response.body.as_slice()),
        (200, b"archive factice".as_slice())
    );
    assert_eq!(response.header("content-type"), Some("application/octet-stream"));
    let disposition = format!("attachment; filename=\"{ARCHIVE}\"");
    assert_eq!(response.header("content-disposition"), Some(disposition.as_str()));
}

#[test]
fn a_missing_archive_is_404_and_a_foreign_name_is_not_a_download() {
    let downloads = TempDir::new("downloads");
    let (_service, context) = with_downloads(Some(&downloads));
    let missing = vaultagent_server::route(&get("/downloads/windows-client.zip"), &context);
    assert_eq!(
        (
            missing.status,
            missing.body_json().unwrap()["error"].as_str().map(str::to_owned)
        ),
        (404, Some("download-unavailable".to_owned()))
    );
    let outside = vaultagent_server::route(&get("/downloads/../secret"), &context);
    assert_eq!(
        (
            outside.status,
            outside.body_json().unwrap()["error"].as_str().map(str::to_owned)
        ),
        (403, Some("origin-refused".to_owned()))
    );
}

#[test]
fn the_style_sheet_carries_its_type_and_the_script_free_policy() {
    let (_service, context) = with_downloads(None);
    let response = vaultagent_server::route(&get("/style.css"), &context);
    assert_eq!(
        (response.status, response.header("content-type")),
        (200, Some("text/css; charset=utf-8"))
    );
    assert_eq!(response.header("referrer-policy"), Some("no-referrer"));
    let policy = response.header("content-security-policy").unwrap();
    assert!(policy.contains("frame-ancestors 'none'") && !policy.contains("script-src"));
    assert!(String::from_utf8(response.body).unwrap().contains(".status"));
}

#[test]
fn the_old_front_scripts_and_page_files_are_gone() {
    let (_service, context) = with_downloads(None);
    for path in ["/app.js", "/index.html"] {
        let response = vaultagent_server::route(&get(path), &context);
        assert_eq!(
            (
                response.status,
                response.body_json().unwrap()["error"].as_str().map(str::to_owned)
            ),
            (404, Some("not-found".to_owned()))
        );
    }
}
