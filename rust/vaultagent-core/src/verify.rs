//! Plan de vérification par relecture d'une création ou d'une modification de fiche : comparaison de la fiche relue
//! à la fiche attendue, historique des mots de passe compris. Aucun moteur : la boucle d'essais est ailleurs.

use crate::canonical_json::canonical_json;
use crate::jsval::{small_int, truthy};
use serde_json::Value;
use std::time::Duration;

/// Nombre d'essais de relecture avant `write-indeterminate`.
pub const ATTEMPTS: u32 = 5;

/// Délai entre deux relectures.
pub const RETRY_DELAY: Duration = Duration::from_millis(200);

/// Nombre d'anciens mots de passe que le serveur conserve derrière le plus récent.
const HISTORY_TAIL: usize = 4;

/// Égalité profonde au sens de Node : formes canoniques (clés triées, `null` retirés, nombres JavaScript) égales.
#[must_use]
pub fn same_json(left: &Value, right: &Value) -> bool {
    canonical_json(left) == canonical_json(right)
}

/// Retire les champs que le serveur entretient seul, puis met la fiche sous forme canonique.
///
/// Paramètres : la fiche, et si le mot de passe a changé (l'historique et la date du mot de passe sont alors
/// retirés). `revisionDate`, `url` des pièces jointes et `uriChecksum` des adresses sont toujours retirés.
/// Retour : la copie comparable.
#[must_use]
pub fn comparable(item: &Value, password_changed: bool) -> Value {
    let mut copy = item.clone();
    let Some(object) = copy.as_object_mut() else {
        return copy;
    };
    object.remove("revisionDate");
    if password_changed {
        object.remove("passwordHistory");
        if let Some(login) = object.get_mut("login").and_then(Value::as_object_mut) {
            login.remove("passwordRevisionDate");
        }
    }
    strip_entries(object.get_mut("attachments"), "url");
    let uris = object
        .get_mut("login")
        .and_then(Value::as_object_mut)
        .and_then(|login| login.get_mut("uris"));
    strip_entries(uris, "uriChecksum");
    copy
}

/// Retire une clé de chaque objet d'une liste.
fn strip_entries(list: Option<&mut Value>, key: &str) {
    for entry in list.and_then(Value::as_array_mut).into_iter().flatten() {
        if let Some(object) = entry.as_object_mut() {
            object.remove(key);
        }
    }
}

/// Texte d'un champ au sens de `valeur ?? ''` : absent ou `null` devient la chaîne vide.
fn text_or_empty(value: Option<&Value>) -> Value {
    match value {
        None | Some(Value::Null) => Value::String(String::new()),
        Some(other) => other.clone(),
    }
}

/// Valeur d'un champ au sens de `valeur ?? null`.
fn or_null(value: Option<&Value>) -> Value {
    value.cloned().unwrap_or(Value::Null)
}

/// Liste de chaînes triée d'un champ `collectionIds` (absent ou non tableau : liste vide).
fn sorted_ids(value: Option<&Value>) -> Vec<Value> {
    let mut ids = value.and_then(Value::as_array).cloned().unwrap_or_default();
    ids.sort_by_key(canonical_json);
    ids
}

/// Adresses (`uri`) de la connexion d'une fiche.
fn uris_of(login: Option<&Value>) -> Vec<Value> {
    let entries = login.and_then(|l| l.get("uris")).and_then(Value::as_array);
    entries
        .into_iter()
        .flatten()
        .map(|entry| or_null(entry.get("uri")))
        .collect()
}

/// Indique si la fiche relue correspond à la fiche demandée à la création.
///
/// Paramètres : la fiche relue, le champ `item` de la requête `create`.
/// Retour : `true` si organisation, collections, type, nom, dossier, notes et, pour une connexion, identifiant,
/// mot de passe, TOTP et adresses correspondent.
#[must_use]
pub fn matches_created(saved: &Value, item: &Value) -> bool {
    let same = |key: &str| or_null(saved.get(key)) == or_null(item.get(key));
    let org_and_folder = same("organizationId") && same("folderId");
    let collections = sorted_ids(saved.get("collectionIds")) == sorted_ids(item.get("collectionIds"));
    let kind = saved.get("type").and_then(small_int) == item.get("type").and_then(small_int);
    let notes = text_or_empty(saved.get("notes")) == text_or_empty(item.get("notes"));
    if !(org_and_folder && collections && kind && notes && same("name")) {
        return false;
    }
    if item.get("type").and_then(small_int) != Some(1) {
        return true;
    }
    let (saved_login, wanted) = (saved.get("login"), item.get("login"));
    let fields = ["username", "password", "totp"].iter().all(|key| {
        text_or_empty(saved_login.and_then(|l| l.get(*key))) == text_or_empty(wanted.and_then(|l| l.get(*key)))
    });
    fields && uris_of(saved_login) == uris_of(wanted)
}

/// Mot de passe de la connexion d'une fiche, au sens de `login.password ?? ''`.
fn login_password(item: &Value) -> Value {
    text_or_empty(item.get("login").and_then(|login| login.get("password")))
}

/// Contrôle l'historique des mots de passe d'une fiche relue : l'ancien mot de passe en tête, puis les quatre
/// précédents de la fiche d'avant.
fn history_is_updated(before: &Value, saved: &Value) -> bool {
    let list = |item: &Value| {
        item.get("passwordHistory")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let saved_history = list(saved);
    let Some((head, rest)) = saved_history.split_first() else {
        return false;
    };
    let previous: Vec<Value> = list(before).into_iter().take(HISTORY_TAIL).collect();
    head.get("password") == Some(&login_password(before))
        && same_json(&Value::Array(rest.to_vec()), &Value::Array(previous))
}

/// Indique si la fiche relue après une modification est celle qu'on attendait.
///
/// Paramètres : la fiche avant écriture, la fiche attendue (patch appliqué), la fiche relue, le patch.
/// Retour : `true` si les fiches comparables sont égales et, quand le mot de passe change et qu'il y en avait un,
/// si l'historique contient l'ancien mot de passe en tête puis les 4 précédents.
#[must_use]
pub fn verify_update(before: &Value, expected: &Value, saved: &Value, patch: &Value) -> bool {
    let changed = patch
        .get("password")
        .is_some_and(|wanted| login_password(before) != *wanted);
    let same = same_json(&comparable(saved, changed), &comparable(expected, changed));
    let had_password = truthy(&login_password(before));
    same && (!changed || !had_password || history_is_updated(before, saved))
}
