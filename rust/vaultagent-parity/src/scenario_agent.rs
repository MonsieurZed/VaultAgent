//! Routes agent d'un coffre déverrouillé : lecture, écriture d'une fiche factice, enveloppe `seal` et partage.

use crate::run::{Run, field, object};
use serde_json::Value;
use vaultagent_core::{Envelope, open_envelope};

const ORIGIN: &str = "https://parity.example.invalid";
const FIRST_VALUE: &str = "parity-secret-0001";
const SECOND_VALUE: &str = "parity-secret-0002";

/// Corps d'une écriture `create` de la fiche factice.
fn create_body(operation: &str, tag: &str) -> Value {
    let login = object([
        ("username", Value::from("parity-user")),
        ("password", Value::from(FIRST_VALUE)),
        ("uris", Value::Array(vec![object([("uri", Value::from(ORIGIN))])])),
    ]);
    let item = object([
        ("type", Value::from(1)),
        ("name", Value::from(format!("parity-{tag}"))),
        ("login", login),
    ]);
    object([
        ("operationId", Value::from(operation)),
        ("action", Value::from("create")),
        ("item", item),
    ])
}

/// Corps d'une écriture `update` qui renomme la fiche et change son mot de passe.
fn update_body(operation: &str, id: &str, revision: &str, tag: &str) -> Value {
    let patch = object([
        ("name", Value::from(format!("parity-{tag}-2"))),
        ("password", Value::from(SECOND_VALUE)),
    ]);
    object([
        ("operationId", Value::from(operation)),
        ("action", Value::from("update")),
        ("itemId", Value::from(id)),
        ("expectedRevisionDate", Value::from(revision)),
        ("patch", patch),
    ])
}

/// Joue `seal` puis ouvre l'enveloppe avec la clé de test et enregistre si le secret retrouvé est le bon.
fn seal_and_open(run: &mut Run<'_>, id: &str) {
    let body = object([
        ("itemId", Value::from(id)),
        ("field", Value::from("password")),
        ("origin", Value::from(ORIGIN)),
        ("publicKey", Value::from(run.settings.public_key.as_str())),
    ]);
    let reply = run.agent_post("agent-seal", "seal", &body);
    let opened = serde_json::from_value::<Envelope>(reply)
        .ok()
        .and_then(|envelope| open_envelope(&envelope, &run.settings.key, &mut run.entropy).ok())
        .is_some_and(|plain| plain.as_slice() == SECOND_VALUE.as_bytes());
    run.record("agent-seal-opened", object([("matches", Value::Bool(opened))]));
    let wrong_origin = object([
        ("itemId", Value::from(id)),
        ("field", Value::from("password")),
        ("origin", Value::from("https://autre.example.invalid")),
        ("publicKey", Value::from(run.settings.public_key.as_str())),
    ]);
    run.agent_post("agent-seal-origin-refused", "seal", &wrong_origin);
    let wrong_field = object([
        ("itemId", Value::from(id)),
        ("field", Value::from("notes")),
        ("origin", Value::from(ORIGIN)),
        ("publicKey", Value::from(run.settings.public_key.as_str())),
    ]);
    run.agent_post("agent-seal-invalid-field", "seal", &wrong_field);
}

/// Aperçu de partage vers la première collection de la première organisation, si le coffre en expose une.
fn share_preview(run: &mut Run<'_>, id: &str, sharing: &Value) {
    let organization = sharing.pointer("/organizations/0");
    let organization_id = organization.map_or_else(String::new, |org| field(org, "id"));
    let collection_id = organization
        .and_then(|org| org.pointer("/collections/0"))
        .map_or_else(String::new, |collection| field(collection, "id"));
    let body = object([
        ("action", Value::from("preview")),
        ("itemId", Value::from(id)),
        ("organizationId", Value::from(organization_id.as_str())),
        ("collectionIds", Value::Array(vec![Value::from(collection_id.as_str())])),
    ]);
    run.agent_post("agent-share-preview", "share", &body);
}

/// Joue les routes de lecture et le cycle créer, rejouer, mettre à jour, sceller, supprimer.
///
/// Paramètres : le passage, associé et déverrouillé.
/// Retour : rien.
pub fn routes(run: &mut Run<'_>) {
    run.agent_post("agent-status", "status", &object([]));
    run.agent_post("agent-vaults", "vaults", &object([]));
    run.agent_post("agent-catalog", "catalog", &object([]));
    run.agent_post("agent-write-preview-list", "write-preview", &object([]));
    run.drop_parity_items();
    let tag = run.settings.tag.clone();
    let operation = run.uuid();
    let created = run.agent_post("agent-write-create", "write", &create_body(&operation, &tag));
    run.agent_post("agent-write-create-replayed", "write", &create_body(&operation, &tag));
    let id = field(&created, "id");
    run.agent_post(
        "agent-catalog-search",
        "catalog",
        &object([("search", Value::from(format!("parity-{tag}")))]),
    );
    run.agent_post(
        "agent-write-preview-detail",
        "write-preview",
        &object([("itemId", Value::from(id.as_str()))]),
    );
    let operation = run.uuid();
    let updated = run.agent_post(
        "agent-write-update",
        "write",
        &update_body(&operation, &id, &field(&created, "revisionDate"), &tag),
    );
    seal_and_open(run, &id);
    let sharing = run.agent_post("agent-sharing", "sharing", &object([]));
    share_preview(run, &id, &sharing);
    let operation = run.uuid();
    let delete = object([
        ("operationId", Value::from(operation.as_str())),
        ("action", Value::from("delete")),
        ("itemId", Value::from(id.as_str())),
        ("expectedRevisionDate", Value::from(field(&updated, "revisionDate"))),
    ]);
    run.agent_post("agent-write-delete", "write", &delete);
    run.agent_post("agent-write-preview-trash", "write-preview", &object([]));
    run.drop_parity_items();
}
