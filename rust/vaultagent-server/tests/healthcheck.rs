//! Sonde de santé contre le vrai service `serve` sur un port éphémère.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::TempDir;
use common::service::{ORIGIN, OWNER};
use vaultagent_engine::FakeFactory;
use vaultagent_server::bootstrap::build_context;
use vaultagent_server::healthcheck::probe;
use vaultagent_server::settings::Settings;
use vaultagent_server::transport::serve;

#[test]
fn probe_succeeds_against_a_running_service() {
    let dir = TempDir::new("healthcheck");
    let settings = Settings {
        data: dir.path().join("data"),
        origin: ORIGIN.to_owned(),
        trusted_proxy: "10.0.0.2".to_owned(),
        owner: OWNER.to_owned(),
        downloads: None,
        bind: "127.0.0.1".to_owned(),
        port: 0,
        workers: 2,
    };
    let context = build_context(&settings, Arc::new(FakeFactory::new())).unwrap();
    let running = serve("127.0.0.1:0", settings.workers, &context).unwrap();
    let port = running.address().unwrap().port();
    assert!(probe(port));
}
