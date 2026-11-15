//! Fiche à créer (`item` d'une écriture `create`) : validation de la forme, des bornes et des collections.

use crate::error::{ErrorCode, Result};
use crate::https::HttpsAddress;
use crate::jsval::{only_keys, small_int, truthy, utf16_len};
use crate::name::{BoundedName, ITEM_NAME_MAX};
use crate::uuid::Uuid;
use serde_json::{Map, Value};
use std::fmt;

const ITEM_KEYS: [&str; 7] = [
    "type",
    "name",
    "folderId",
    "notes",
    "login",
    "organizationId",
    "collectionIds",
];
const LOGIN_KEYS: [&str; 4] = ["username", "password", "totp", "uris"];
const NOTES_MAX: usize = 12_000;
const FIELD_MAX: usize = 8192;
const MAX_URIS: usize = 20;
const MAX_COLLECTIONS: usize = 32;

/// Type d'une fiche créée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// Connexion (type 1).
    Login,
    /// Note sécurisée (type 2).
    SecureNote,
}

/// Connexion d'une fiche à créer ; la valeur des secrets n'apparaît jamais dans `Debug`.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct NewLogin {
    /// Identifiant de connexion.
    pub username: Option<String>,
    /// Mot de passe.
    pub password: Option<String>,
    /// Graine ou URI TOTP.
    pub totp: Option<String>,
    /// Adresses HTTPS.
    pub uris: Vec<HttpsAddress>,
}

impl fmt::Debug for NewLogin {
    /// Écrit la présence des champs, jamais leur valeur.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewLogin")
            .field("username", &self.username.is_some())
            .field("password", &self.password.is_some())
            .field("totp", &self.totp.is_some())
            .field("uris", &self.uris.len())
            .finish()
    }
}

/// Fiche à créer, validée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItem {
    /// Type de fiche.
    pub kind: ItemKind,
    /// Nom.
    pub name: BoundedName,
    /// Dossier.
    pub folder_id: Option<Uuid>,
    /// Notes.
    pub notes: Option<String>,
    /// Connexion (type 1 seulement).
    pub login: Option<NewLogin>,
    /// Organisation propriétaire.
    pub organization_id: Option<Uuid>,
    /// Collections (non vide si et seulement si une organisation est donnée).
    pub collection_ids: Vec<Uuid>,
}

/// Lit une chaîne bornée, absente permise ; `null` est refusé comme dans Node.
fn bounded_text(map: &Map<String, Value>, key: &str, max: usize) -> Result<Option<String>> {
    match map.get(key) {
        None => Ok(None),
        Some(Value::String(text)) if utf16_len(text) <= max => Ok(Some(text.clone())),
        Some(_) => Err(ErrorCode::InvalidWrite),
    }
}

/// Lit un UUID optionnel : absent ou `null` donne `None`, tout autre cas invalide est refusé.
pub(crate) fn optional_uuid(map: &Map<String, Value>, key: &str) -> Result<Option<Uuid>> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Uuid::parse(text).map(Some).ok_or(ErrorCode::InvalidWrite),
        Some(_) => Err(ErrorCode::InvalidWrite),
    }
}

/// Lit les collections : de 1 à 32 UUID avec une organisation ; sans organisation, aucune.
fn collections(map: &Map<String, Value>, organization: Option<&Uuid>) -> Result<Vec<Uuid>> {
    let raw = map.get("collectionIds");
    if organization.is_none() {
        return if raw.is_some_and(|v| match v {
            Value::Array(list) => !list.is_empty(),
            other => truthy(other) && !other.is_object(),
        }) {
            Err(ErrorCode::InvalidWrite)
        } else {
            Ok(Vec::new())
        };
    }
    let list = raw
        .and_then(Value::as_array)
        .filter(|list| !list.is_empty() && list.len() <= MAX_COLLECTIONS);
    list.and_then(|list| list.iter().map(|v| v.as_str().and_then(Uuid::parse)).collect())
        .ok_or(ErrorCode::InvalidWrite)
}

/// Lit la connexion d'une fiche de type 1.
fn login(value: Option<&Value>) -> Result<NewLogin> {
    let map = value
        .and_then(|v| only_keys(v, &LOGIN_KEYS))
        .ok_or(ErrorCode::InvalidWrite)?;
    let uris = match map.get("uris") {
        None => Vec::new(),
        Some(Value::Array(list)) if list.len() <= MAX_URIS => list.iter().map(address).collect::<Result<_>>()?,
        Some(_) => return Err(ErrorCode::InvalidWrite),
    };
    Ok(NewLogin {
        username: bounded_text(map, "username", FIELD_MAX)?,
        password: bounded_text(map, "password", FIELD_MAX)?,
        totp: bounded_text(map, "totp", FIELD_MAX)?,
        uris,
    })
}

/// Lit une entrée `{uri}` de la liste d'adresses.
fn address(entry: &Value) -> Result<HttpsAddress> {
    let map = only_keys(entry, &["uri"]).ok_or(ErrorCode::InvalidWrite)?;
    map.get("uri")
        .and_then(Value::as_str)
        .and_then(HttpsAddress::parse)
        .ok_or(ErrorCode::InvalidWrite)
}

impl NewItem {
    /// Valide l'objet `item` d'une écriture `create`.
    ///
    /// Paramètres : la valeur JSON de `item`.
    /// Retour : la fiche validée.
    /// # Errors
    ///
    /// `InvalidWrite` pour toute clé inconnue, type, nom, dossier, organisation, collection, note ou
    /// connexion invalide, ou pour une connexion donnée à une note sécurisée.
    pub fn parse(item: &Value) -> Result<Self> {
        let map = only_keys(item, &ITEM_KEYS).ok_or(ErrorCode::InvalidWrite)?;
        let kind = match map.get("type").and_then(small_int) {
            Some(1) => ItemKind::Login,
            Some(2) => ItemKind::SecureNote,
            _ => return Err(ErrorCode::InvalidWrite),
        };
        let name = map
            .get("name")
            .and_then(Value::as_str)
            .and_then(|text| BoundedName::parse(text, ITEM_NAME_MAX));
        let organization_id = optional_uuid(map, "organizationId")?;
        let login = match (kind, map.get("login")) {
            (ItemKind::Login, value) => Some(login(value)?),
            (ItemKind::SecureNote, None) => None,
            (ItemKind::SecureNote, Some(_)) => return Err(ErrorCode::InvalidWrite),
        };
        Ok(Self {
            kind,
            name: name.ok_or(ErrorCode::InvalidWrite)?,
            folder_id: optional_uuid(map, "folderId")?,
            notes: bounded_text(map, "notes", NOTES_MAX)?,
            login,
            collection_ids: collections(map, organization_id.as_ref())?,
            organization_id,
        })
    }
}
