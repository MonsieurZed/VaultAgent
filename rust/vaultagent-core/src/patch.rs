//! Patch d'écriture d'une fiche : équivalent de `fork/write-patch.mjs`. La fiche (`CipherView` en JSON) est modifiée
//! en place, tous les champs non ciblés sont conservés.

use crate::error::{ErrorCode, Result};
use crate::jsval::{field_truthy, has_control, is_blank, small_int, utf16_len};
use crate::patch_uris::{add_addresses, remove_addresses};
use crate::revision::RevisionDate;
use crate::uuid::Uuid;
use serde_json::{Map, Value};

const ALLOWED: [&str; 8] = [
    "name",
    "folderId",
    "notes",
    "username",
    "password",
    "totp",
    "removeUris",
    "addUris",
];
const LOGIN_FIELDS: [&str; 3] = ["username", "password", "totp"];
const NAME_MAX: usize = 500;
const NOTES_MAX: usize = 12_000;
const FIELD_MAX: usize = 8192;

/// Lit le patch : un objet non vide dont toutes les clés sont permises.
///
/// # Errors
///
/// `InvalidPatch` sinon.
fn patch_fields(patch: &Value) -> Result<&Map<String, Value>> {
    patch
        .as_object()
        .filter(|map| !map.is_empty() && map.keys().all(|key| ALLOWED.contains(&key.as_str())))
        .ok_or(ErrorCode::InvalidPatch)
}

/// Vérifie la révision attendue et l'état de la fiche (propriétaire, corbeille, archive, type).
///
/// # Errors
///
/// `RevisionConflict` si la date diffère, `RequestRefused` si la date de la fiche est illisible,
/// `ItemRefused` si la fiche n'est pas modifiable par ce propriétaire.
fn check_item(view: &Value, expected: &RevisionDate, organization_id: Option<&Uuid>) -> Result<()> {
    let object = view.as_object().ok_or(ErrorCode::ItemRefused)?;
    let current = object
        .get("revisionDate")
        .and_then(Value::as_str)
        .and_then(RevisionDate::parse)
        .ok_or(ErrorCode::RequestRefused)?;
    if !current.same_instant(expected) {
        return Err(ErrorCode::RevisionConflict);
    }
    let owner = match object.get("organizationId") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.as_str()),
        Some(_) => return Err(ErrorCode::ItemRefused),
    };
    let flagged = ["isDeleted", "isArchived", "deletedDate", "archivedDate"]
        .iter()
        .any(|key| field_truthy(object, key));
    let kind_ok = object
        .get("type")
        .and_then(small_int)
        .is_some_and(|kind| kind == 1 || kind == 2);
    if owner != organization_id.map(Uuid::as_str) || flagged || !kind_ok {
        return Err(ErrorCode::ItemRefused);
    }
    Ok(())
}

/// Valide la valeur d'un champ texte du patch.
///
/// # Errors
///
/// `InvalidPatch` si ce n'est pas une chaîne, si elle dépasse sa borne, ou si un nom est vide ou contient
/// un caractère de contrôle.
fn check_text(key: &str, value: &Value) -> Result<()> {
    let text = value.as_str().ok_or(ErrorCode::InvalidPatch)?;
    let max = if key == "notes" { NOTES_MAX } else { FIELD_MAX };
    let bad_name = key == "name" && (is_blank(text) || utf16_len(text) > NAME_MAX || has_control(text));
    if utf16_len(text) > max || bad_name {
        Err(ErrorCode::InvalidPatch)
    } else {
        Ok(())
    }
}

/// Applique un champ scalaire du patch (hors adresses) à la fiche.
///
/// # Errors
///
/// `InvalidPatch` pour une valeur invalide, `ItemRefused` pour un champ de connexion sur une fiche
/// qui n'est pas une connexion.
fn set_field(view: &mut Value, key: &str, value: &Value) -> Result<()> {
    if key == "folderId" {
        let valid = value.is_null() || value.as_str().and_then(Uuid::parse).is_some();
        return if valid {
            assign(view, key, value)
        } else {
            Err(ErrorCode::InvalidPatch)
        };
    }
    check_text(key, value)?;
    if !LOGIN_FIELDS.contains(&key) {
        return assign(view, key, value);
    }
    let is_login = view.get("type").and_then(small_int) == Some(1);
    let login = view
        .get_mut("login")
        .and_then(Value::as_object_mut)
        .filter(|_| is_login)
        .ok_or(ErrorCode::ItemRefused)?;
    login.insert(key.to_owned(), value.clone());
    Ok(())
}

/// Écrit une clé de premier niveau de la fiche.
fn assign(view: &mut Value, key: &str, value: &Value) -> Result<()> {
    let object = view.as_object_mut().ok_or(ErrorCode::ItemRefused)?;
    object.insert(key.to_owned(), value.clone());
    Ok(())
}

/// Applique un patch contrôlé à une fiche, après vérification de la révision et du propriétaire.
///
/// Les retraits d'adresses passent avant les ajouts, pour remplacer une adresse en une seule écriture. Les autres
/// champs sont appliqués dans l'ordre alphabétique des clés. L'application est atomique : en cas d'erreur, la fiche
/// est inchangée.
///
/// Paramètres : la fiche à modifier, la date de révision attendue, le patch (`name`, `folderId`, `notes`, `username`,
/// `password`, `totp`, `removeUris`, `addUris`) et l'organisation de la fiche (`None` si personnelle).
/// Retour : rien, la fiche est modifiée.
/// # Errors
///
/// `InvalidPatch`, `RevisionConflict`, `ItemRefused` (voir le protocole).
pub fn apply_write_patch(
    view: &mut Value,
    expected_revision: &RevisionDate,
    patch: &Value,
    organization_id: Option<&Uuid>,
) -> Result<()> {
    let fields = patch_fields(patch)?;
    check_item(view, expected_revision, organization_id)?;
    let mut work = view.clone();
    if let Some(value) = fields.get("removeUris") {
        remove_addresses(&mut work, value)?;
    }
    if let Some(value) = fields.get("addUris") {
        add_addresses(&mut work, value)?;
    }
    for (key, value) in fields
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "removeUris" | "addUris"))
    {
        set_field(&mut work, key, value)?;
    }
    *view = work;
    Ok(())
}
