//! Empreinte d'idempotence d'une requête : HMAC-SHA-256 de sa forme canonique, clé machine de 32 octets.

use crate::canonical_json::{canonical_json, push_hex_byte};
use hmac::digest::{Key, KeyInit};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;

/// Calcule l'empreinte d'une requête d'écriture ou de partage.
///
/// La requête est celle d'origine, sans `vaultId` (déjà retiré par l'appelant, comme Node).
///
/// Paramètres : la clé machine, la requête JSON.
/// Retour : le HMAC-SHA-256 de `canonical_json(request)` en hexadécimal minuscule (64 caractères).
#[must_use]
pub fn fingerprint(machine_key: &[u8; 32], request: &Value) -> String {
    let mut block = Key::<Hmac<Sha256>>::default();
    block[..32].copy_from_slice(machine_key);
    let mut mac = <Hmac<Sha256> as KeyInit>::new(&block);
    mac.update(canonical_json(request).as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in mac.finalize().into_bytes() {
        push_hex_byte(&mut hex, byte);
    }
    hex
}
