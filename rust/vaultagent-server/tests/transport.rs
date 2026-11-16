//! Essai de bout en bout : le vrai transport `tiny_http` sur `127.0.0.1:0`, le faux moteur et un client `TcpStream`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;

use common::service::{ORIGIN, OWNER};
use common::{EMAIL, FICTITIOUS, SERVER, TempDir, connectable};
use serde_json::{Value, json};
use vaultagent_engine::FakeFactory;
use vaultagent_server::bootstrap::build_context;
use vaultagent_server::settings::Settings;
use vaultagent_server::transport::{Running, serve};

/// Service en écoute sur un port libre, avec son dossier de données temporaire.
struct Live {
    running: Running,
    factory: FakeFactory,
    _dir: TempDir,
}

impl Live {
    /// Démarre le service ; `proxy` est l'adresse de proxy de confiance déclarée.
    fn start(proxy: &str) -> Self {
        let dir = TempDir::new("transport");
        let settings = Settings {
            data: dir.path().join("data"),
            origin: ORIGIN.to_owned(),
            trusted_proxy: proxy.to_owned(),
            owner: OWNER.to_owned(),
            downloads: None,
            bind: "127.0.0.1".to_owned(),
            port: 0,
            workers: 2,
        };
        let factory = FakeFactory::new();
        let context = build_context(&settings, Arc::new(factory.clone())).unwrap();
        let running = serve("127.0.0.1:0", settings.workers, &context).unwrap();
        Self {
            running,
            factory,
            _dir: dir,
        }
    }

    /// Adresse d'écoute.
    fn address(&self) -> SocketAddr {
        self.running.address().unwrap()
    }
}

/// Réponse lue : statut, en-têtes en minuscules et corps.
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    /// Corps lu comme JSON.
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }

    /// Valeur d'un en-tête.
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Envoie un texte HTTP brut et lit la réponse jusqu'à la fermeture.
fn exchange(address: SocketAddr, raw: &[u8]) -> Reply {
    let mut stream = TcpStream::connect(address).unwrap();
    stream.write_all(raw).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let split = bytes.windows(4).position(|window| window == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&bytes[..split]).into_owned();
    let mut lines = head.lines();
    let status = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Reply {
        status,
        headers,
        body: bytes[split + 4..].to_vec(),
    }
}

/// Requête HTTP avec corps JSON facultatif.
fn call(address: SocketAddr, method: &str, path: &str, extra: &[(&str, &str)], body: Option<&Value>) -> Reply {
    let payload = body.map_or_default(Value::to_string);
    let mut lines = vec![
        format!("{method} {path} HTTP/1.1"),
        "Host: test".to_owned(),
        "Connection: close".to_owned(),
    ];
    lines.extend(extra.iter().map(|(name, value)| format!("{name}: {value}")));
    if body.is_some() {
        lines.push("Content-Type: application/json".to_owned());
        lines.push(format!("Content-Length: {}", payload.len()));
    }
    let raw = format!("{}\r\n\r\n{payload}", lines.join("\r\n"));
    exchange(address, raw.as_bytes())
}

/// En-têtes du propriétaire pour une action `POST`.
fn as_owner() -> [(&'static str, &'static str); 2] {
    [("x-authentik-username", OWNER), ("origin", ORIGIN)]
}

#[test]
fn full_flow_over_real_sockets() {
    let live = Live::start("127.0.0.1");
    let address = live.address();
    let health = call(address, "GET", "/healthz", &[], None);
    assert_eq!((health.status, health.json()), (200, json!({"alive": true})));
    assert_eq!(health.header("cache-control"), Some("no-store"));
    connectable(&live.factory);
    let account = json!({"server": SERVER, "email": EMAIL, "password": FICTITIOUS, "timeoutMinutes": 15});
    assert_eq!(
        call(address, "POST", "/api/admin/configure", &as_owner(), Some(&account)).status,
        200
    );
    let issued = call(
        address,
        "POST",
        "/api/admin/pairing",
        &as_owner(),
        Some(&json!({"name": "Poste"})),
    );
    let code = issued.json()["code"].as_str().unwrap().to_owned();
    let paired = call(address, "POST", "/api/pair", &[], Some(&json!({"code": code})));
    assert_eq!(paired.status, 200);
    let bearer = format!("Bearer {}", paired.json()["token"].as_str().unwrap());
    let status = call(
        address,
        "POST",
        "/api/agent/status",
        &[("authorization", &bearer)],
        Some(&json!({})),
    );
    assert_eq!((status.status, status.json()["unlocked"].clone()), (200, json!(true)));
    let front = call(address, "GET", "/", &[("x-authentik-username", OWNER)], None);
    assert_eq!(front.status, 200);
    assert_eq!(front.header("content-type"), Some("text/html; charset=utf-8"));
    assert!(
        front
            .header("content-security-policy")
            .is_some_and(|policy| policy.starts_with("default-src 'none'"))
    );
    let html = String::from_utf8(front.body.clone()).unwrap();
    assert!(html.contains("Coffre disponible pour les clients") && html.contains(SERVER) && !html.contains("<script"));
    live.running.shutdown();
}

#[test]
fn another_peer_than_the_proxy_only_reaches_healthz() {
    let live = Live::start("10.9.9.9");
    let address = live.address();
    assert_eq!(call(address, "GET", "/healthz", &[], None).status, 200);
    let refused = call(address, "POST", "/api/agent/status", &[], Some(&json!({})));
    assert_eq!(
        (refused.status, refused.json()["error"].clone()),
        (403, json!("proxy-required"))
    );
    live.running.shutdown();
}

#[test]
fn limits_on_body_and_header_count_apply() {
    let live = Live::start("127.0.0.1");
    let address = live.address();
    let padding = "x".repeat(30_000);
    let big = call(
        address,
        "POST",
        "/api/pair",
        &[],
        Some(&json!({"action": "request", "padding": padding})),
    );
    assert_eq!((big.status, big.json()["error"].clone()), (400, json!("request-size")));
    let many: Vec<(String, String)> = (0..45)
        .map(|index| (format!("x-extra-{index}"), "1".to_owned()))
        .collect();
    let borrowed: Vec<(&str, &str)> = many
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    assert_eq!(call(address, "GET", "/healthz", &borrowed, None).status, 431);
    live.running.shutdown();
}

#[test]
fn an_idle_connection_does_not_block_other_requests() {
    let live = Live::start("127.0.0.1");
    let address = live.address();
    let idle = TcpStream::connect(address).unwrap();
    let slow = TcpStream::connect(address).unwrap();
    for _ in 0..4 {
        assert_eq!(call(address, "GET", "/healthz", &[], None).status, 200);
    }
    drop((idle, slow));
    live.running.shutdown();
}
