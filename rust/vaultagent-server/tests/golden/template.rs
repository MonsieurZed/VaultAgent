//! Gabarits des cas dorés : `{{token}}`, `{{origin}}`, `{{owner}}`, `{{itemId}}`, `{{rsaPublicKey}}`,
//! `{{operationId}}` et `{{stepN.<clé>}}`. Les valeurs génériques (`{{uuid}}`…) restent intactes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::Value;

use crate::common::service::{ITEM_ID, ORIGIN, OWNER};

/// Valeurs substituées dans un cas.
pub struct Values {
    pub token: String,
    pub steps: Vec<Value>,
}

/// UUID fixe de l'opération du cas.
pub const OPERATION_ID: &str = "00000000-0000-4000-8000-0000000000b1";

/// Clé publique RSA de test (SPKI DER en base64), factice et publique.
fn rsa_public_key() -> String {
    include_str!("../fixtures/rsa-public.txt").trim().to_owned()
}

/// Valeur d'un gabarit, ou `None` s'il est générique ou inconnu.
fn lookup(name: &str, values: &Values) -> Option<String> {
    match name {
        "token" => Some(format!("Bearer {}", values.token)),
        "origin" => Some(ORIGIN.to_owned()),
        "owner" => Some(OWNER.to_owned()),
        "itemId" => Some(ITEM_ID.to_owned()),
        "rsaPublicKey" => Some(rsa_public_key()),
        "operationId" => Some(OPERATION_ID.to_owned()),
        _ => {
            let (step, key) = name.strip_prefix("step")?.split_once('.')?;
            let index: usize = step.parse().ok()?;
            let body = values.steps.get(index.checked_sub(1)?)?;
            body.get(key)?.as_str().map(str::to_owned)
        }
    }
}

/// Remplace les gabarits d'une chaîne.
fn fill(text: &str, values: &Values) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let Some(end) = rest[start..].find("}}") else { break };
        out.push_str(&rest[..start]);
        let name = &rest[start + 2..start + end];
        match lookup(name, values) {
            Some(found) => out.push_str(&found),
            None => out.push_str(&rest[start..start + end + 2]),
        }
        rest = &rest[start + end + 2..];
    }
    out.push_str(rest);
    out
}

/// Remplace les gabarits dans toutes les chaînes d'une valeur JSON.
pub fn substitute(value: &Value, values: &Values) -> Value {
    match value {
        Value::String(text) => Value::String(fill(text, values)),
        Value::Array(list) => Value::Array(list.iter().map(|item| substitute(item, values)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), substitute(item, values)))
                .collect(),
        ),
        other => other.clone(),
    }
}
