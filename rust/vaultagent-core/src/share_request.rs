//! Requête de partage (`POST /api/agent/share`) : lecture et validation du JSON avec les règles de `shareItem`.
//! La requête d'origine reste accessible pour l'empreinte d'idempotence (toutes ses clés, sans `vaultId`).

use crate::error::{ErrorCode, Result};
use crate::jsval::{has_control, is_blank, only_keys, trim, utf16_len};
use crate::uuid::Uuid;
use serde_json::{Map, Value};

const KEYS: [&str; 8] = [
    "action",
    "itemId",
    "organizationId",
    "collectionIds",
    "operationId",
    "expectedRevisionDate",
    "confirmOwnershipTransfer",
    "name",
];

/// Nombre maximal de collections d'un partage.
pub const MAX_COLLECTIONS: usize = 32;

/// Longueur maximale du nom d'une collection, en unités UTF-16.
pub const COLLECTION_NAME_MAX: usize = 200;

/// Action de partage demandée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareAction {
    /// Aperçu sans écriture.
    Preview,
    /// Transfert de la fiche vers l'organisation.
    Apply,
    /// Création d'une collection.
    CreateCollection,
}

impl ShareAction {
    /// Nom de l'action dans le protocole.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Apply => "apply",
            Self::CreateCollection => "create-collection",
        }
    }
}

/// Requête de partage validée.
#[derive(Debug, Clone, PartialEq)]
pub struct ShareRequest {
    action: ShareAction,
    organization_id: Uuid,
    item_id: Option<Uuid>,
    collection_ids: Vec<Uuid>,
    operation_id: Option<Uuid>,
    expected_revision: Option<Value>,
    confirmed: bool,
    name: Option<String>,
    raw: Value,
}

/// Lit un champ UUID obligatoire.
fn uuid_field(map: &Map<String, Value>, key: &str) -> Result<Uuid> {
    map.get(key)
        .and_then(Value::as_str)
        .and_then(Uuid::parse)
        .ok_or(ErrorCode::InvalidShare)
}

/// Lit `collectionIds` : 1 à 32 UUID.
fn collections(map: &Map<String, Value>) -> Result<Vec<Uuid>> {
    let list = map
        .get("collectionIds")
        .and_then(Value::as_array)
        .filter(|list| !list.is_empty() && list.len() <= MAX_COLLECTIONS)
        .ok_or(ErrorCode::InvalidShare)?;
    list.iter()
        .map(|value| value.as_str().and_then(Uuid::parse).ok_or(ErrorCode::InvalidShare))
        .collect()
}

/// Lit le nom d'une collection : non vide, 200 unités UTF-16 au plus, sans contrôle ; rendu rogné.
fn collection_name(map: &Map<String, Value>) -> Result<String> {
    let text = map
        .get("name")
        .and_then(Value::as_str)
        .filter(|text| !is_blank(text) && utf16_len(text) <= COLLECTION_NAME_MAX && !has_control(text))
        .ok_or(ErrorCode::InvalidShare)?;
    Ok(trim(text).to_owned())
}

impl ShareRequest {
    /// Valide une requête de partage.
    ///
    /// Paramètres : le corps JSON de la requête, `vaultId` déjà retiré.
    /// Retour : la requête validée, qui garde une copie du JSON d'origine.
    /// # Errors
    ///
    /// `InvalidShare` pour toute clé inconnue, action inconnue, identifiant invalide, collections hors bornes,
    /// nom de collection invalide, ou `operationId` absent hors aperçu.
    pub fn parse(raw: &Value) -> Result<Self> {
        let map = only_keys(raw, &KEYS).ok_or(ErrorCode::InvalidShare)?;
        let action = match map.get("action").and_then(Value::as_str) {
            Some("preview") => ShareAction::Preview,
            Some("apply") => ShareAction::Apply,
            Some("create-collection") => ShareAction::CreateCollection,
            _ => return Err(ErrorCode::InvalidShare),
        };
        let organization_id = uuid_field(map, "organizationId")?;
        let creating = action == ShareAction::CreateCollection;
        let (item_id, collection_ids) = if creating {
            (None, Vec::new())
        } else {
            (Some(uuid_field(map, "itemId")?), collections(map)?)
        };
        let name = if creating { Some(collection_name(map)?) } else { None };
        let operation_id = if action == ShareAction::Preview {
            None
        } else {
            Some(uuid_field(map, "operationId")?)
        };
        Ok(Self {
            action,
            organization_id,
            item_id,
            collection_ids,
            operation_id,
            expected_revision: map.get("expectedRevisionDate").cloned(),
            confirmed: map.get("confirmOwnershipTransfer") == Some(&Value::Bool(true)),
            name,
            raw: raw.clone(),
        })
    }

    /// Action demandée.
    #[must_use]
    pub fn action(&self) -> ShareAction {
        self.action
    }

    /// Organisation visée.
    #[must_use]
    pub fn organization_id(&self) -> &Uuid {
        &self.organization_id
    }

    /// Fiche à partager (`preview` et `apply`).
    #[must_use]
    pub fn item_id(&self) -> Option<&Uuid> {
        self.item_id.as_ref()
    }

    /// Collections de destination, doublons conservés (vide pour `create-collection`).
    #[must_use]
    pub fn collection_ids(&self) -> &[Uuid] {
        &self.collection_ids
    }

    /// Identifiant d'opération (`apply` et `create-collection`).
    #[must_use]
    pub fn operation_id(&self) -> Option<&Uuid> {
        self.operation_id.as_ref()
    }

    /// Date de révision attendue, telle que reçue.
    #[must_use]
    pub fn expected_revision(&self) -> Option<&Value> {
        self.expected_revision.as_ref()
    }

    /// Vrai si le transfert de propriété est confirmé (`confirmOwnershipTransfer` valant exactement `true`).
    #[must_use]
    pub fn confirmed(&self) -> bool {
        self.confirmed
    }

    /// Nom rogné de la collection à créer.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Requête JSON d'origine, base de l'empreinte d'idempotence.
    #[must_use]
    pub fn raw(&self) -> &Value {
        &self.raw
    }
}
