//! Retrait et ajout d'adresses dans la connexion d'une fiche, pour `removeUris` et `addUris` d'un patch d'écriture.

use crate::error::{ErrorCode, Result};
use crate::https::{HttpsAddress, MAX_ADDRESS_LENGTH};
use crate::jsval::{object, small_int, utf16_len};
use serde_json::{Map, Value};

/// Nombre maximal d'adresses d'une fiche.
pub const MAX_URIS: usize = 20;

/// Rend la connexion d'une fiche de type 1 et la liste d'adresses du patch, après contrôle de forme.
///
/// Paramètres : la fiche, la valeur du patch et le contrôle de chaque adresse.
/// Retour : la connexion modifiable et les adresses du patch.
/// # Errors
///
/// `InvalidPatch` si la fiche n'est pas une connexion, ou si la liste est absente, vide, trop longue
/// ou contient une adresse refusée.
fn target<'a>(
    view: &'a mut Value,
    value: &'a Value,
    accepted: fn(&str) -> bool,
) -> Result<(&'a mut Map<String, Value>, Vec<&'a str>)> {
    let is_login = view.get("type").and_then(small_int) == Some(1);
    let login = view
        .get_mut("login")
        .and_then(Value::as_object_mut)
        .filter(|_| is_login);
    let list = value
        .as_array()
        .filter(|list| !list.is_empty() && list.len() <= MAX_URIS);
    let texts: Option<Vec<&str>> =
        list.and_then(|list| list.iter().map(|v| v.as_str().filter(|t| accepted(t))).collect());
    match (login, texts) {
        (Some(login), Some(texts)) => Ok((login, texts)),
        _ => Err(ErrorCode::InvalidPatch),
    }
}

/// Liste courante des adresses de la connexion, vide si absente.
///
/// # Errors
///
/// `ItemRefused` si `uris` n'est ni absent, ni `null`, ni un tableau.
fn current_uris(login: &Map<String, Value>) -> Result<Vec<Value>> {
    match login.get("uris") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(list)) => Ok(list.clone()),
        Some(_) => Err(ErrorCode::ItemRefused),
    }
}

/// Adresse enregistrée d'une entrée de la liste.
fn entry_uri(entry: &Value) -> Option<&str> {
    entry.get("uri").and_then(Value::as_str)
}

/// Accepte toute chaîne non vide d'au plus 2048 unités UTF-16 (désignation d'une adresse à retirer).
fn named(text: &str) -> bool {
    !text.is_empty() && utf16_len(text) <= MAX_ADDRESS_LENGTH
}

/// Accepte une adresse HTTPS de fiche.
fn https(text: &str) -> bool {
    HttpsAddress::parse(text).is_some()
}

/// Retire des adresses d'une connexion ; chacune doit exister, les autres entrées restent intactes.
///
/// Paramètres : la fiche et la valeur `removeUris` du patch.
/// Retour : rien, la fiche est modifiée.
/// # Errors
///
/// `InvalidPatch` pour une forme invalide ou une adresse absente de la fiche.
pub fn remove_addresses(view: &mut Value, value: &Value) -> Result<()> {
    let (login, gone) = target(view, value, named)?;
    let current = current_uris(login)?;
    if !gone
        .iter()
        .all(|uri| current.iter().any(|entry| entry_uri(entry) == Some(uri)))
    {
        return Err(ErrorCode::InvalidPatch);
    }
    let kept: Vec<Value> = current
        .into_iter()
        .filter(|entry| !entry_uri(entry).is_some_and(|uri| gone.contains(&uri)))
        .collect();
    login.insert("uris".to_owned(), Value::Array(kept));
    Ok(())
}

/// Ajoute des adresses à une connexion sans retirer ni modifier celles qui existent ; doublons ignorés.
///
/// Paramètres : la fiche et la valeur `addUris` du patch.
/// Retour : rien, la fiche est modifiée ; chaque entrée ajoutée vaut `{uri, match: null}`.
/// # Errors
///
/// `InvalidPatch` pour une forme invalide ou plus de 20 adresses au total.
pub fn add_addresses(view: &mut Value, value: &Value) -> Result<()> {
    let (login, wanted) = target(view, value, https)?;
    let mut current = current_uris(login)?;
    let mut added: Vec<&str> = Vec::new();
    for uri in wanted {
        let present = current.iter().any(|entry| entry_uri(entry) == Some(uri));
        if !present && !added.contains(&uri) {
            added.push(uri);
        }
    }
    if current.len() + added.len() > MAX_URIS {
        return Err(ErrorCode::InvalidPatch);
    }
    current.extend(
        added
            .into_iter()
            .map(|uri| object(vec![("uri", Value::from(uri)), ("match", Value::Null)])),
    );
    login.insert("uris".to_owned(), Value::Array(current));
    Ok(())
}
