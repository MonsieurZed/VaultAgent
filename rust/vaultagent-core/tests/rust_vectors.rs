//! Vecteurs produits par Rust (graines fixes) puis ouverts avec succès par Node : la sortie reste stable.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value
)]

use serde_json::Value;
use vaultagent_core::{SeededEntropy, protect, seal};

const FIXTURE: &str = include_str!("fixtures/crypto.json");

#[test]
fn rust_output_matches_the_vectors_node_opened() {
    let data: Value = serde_json::from_str(FIXTURE).unwrap();
    let rust = &data["rust"];
    let public = data["seal"]["publicKey"].as_str().unwrap();
    let sealed = seal(
        rust["sealClear"].as_str().unwrap().as_bytes(),
        public,
        &mut SeededEntropy::new(rust["sealSeed"].as_u64().unwrap()),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&sealed).unwrap(), rust["envelope"]);
    let key: Vec<u8> = (0..32u8).collect();
    let stored = protect(
        &key,
        rust["protectClear"].as_str().unwrap().as_bytes(),
        &mut SeededEntropy::new(rust["protectSeed"].as_u64().unwrap()),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&stored).unwrap(), rust["protected"]);
}
