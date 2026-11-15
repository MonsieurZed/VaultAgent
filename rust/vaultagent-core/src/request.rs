//! Requête d'écriture (`POST /api/agent/write`) : lecture et validation du JSON avec les règles de `validate` et
//! `validateLifecycle` de Node. La requête d'origine reste accessible pour l'empreinte d'idempotence.

use crate::error::{ErrorCode, Result};
use crate::jsval::only_keys;
use crate::name::BoundedName;
use crate::new_item::NewItem;
use crate::request_actions as actions;
use crate::revision::RevisionDate;
use crate::uuid::Uuid;
use serde_json::Value;
use std::fmt;

const KEYS: [&str; 8] = [
    "operationId",
    "action",
    "itemId",
    "folderId",
    "expectedRevisionDate",
    "item",
    "patch",
    "organizationId",
];

/// Action demandée, avec ses champs validés.
#[derive(Clone, PartialEq)]
pub enum WriteAction {
    /// Création d'une fiche.
    Create {
        /// Fiche à créer.
        item: NewItem,
    },
    /// Modification ciblée d'une fiche.
    Update {
        /// Fiche visée.
        item_id: Uuid,
        /// Révision attendue.
        expected_revision: RevisionDate,
        /// Patch, validé par application à une fiche témoin.
        patch: Value,
    },
    /// Création d'un dossier.
    CreateFolder {
        /// Nom du dossier.
        name: BoundedName,
    },
    /// Mise à la corbeille.
    Delete {
        /// Fiche visée.
        item_id: Uuid,
        /// Révision attendue.
        expected_revision: RevisionDate,
    },
    /// Restauration depuis la corbeille.
    Restore {
        /// Fiche visée.
        item_id: Uuid,
        /// Révision attendue.
        expected_revision: RevisionDate,
    },
    /// Renommage d'un dossier.
    UpdateFolder {
        /// Dossier visé.
        folder_id: Uuid,
        /// Nouveau nom.
        name: BoundedName,
    },
    /// Suppression d'un dossier.
    DeleteFolder {
        /// Dossier visé.
        folder_id: Uuid,
    },
}

impl WriteAction {
    /// Nom de l'action dans le protocole (`create`, `update`, `create-folder`, …).
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Create { .. } => "create",
            Self::Update { .. } => "update",
            Self::CreateFolder { .. } => "create-folder",
            Self::Delete { .. } => "delete",
            Self::Restore { .. } => "restore",
            Self::UpdateFolder { .. } => "update-folder",
            Self::DeleteFolder { .. } => "delete-folder",
        }
    }
}

impl fmt::Debug for WriteAction {
    /// Écrit seulement le nom de l'action, jamais les valeurs de la requête.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WriteAction({})", self.name())
    }
}

/// Requête d'écriture validée.
#[derive(Clone, PartialEq)]
pub struct WriteRequest {
    operation_id: Uuid,
    organization_id: Option<Uuid>,
    action: WriteAction,
    raw: Value,
}

impl fmt::Debug for WriteRequest {
    /// Écrit l'opération et l'action, jamais les valeurs de la requête.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WriteRequest({}, {})", self.operation_id, self.action.name())
    }
}

impl WriteRequest {
    /// Valide une requête d'écriture.
    ///
    /// Paramètres : le corps JSON de la requête.
    /// Retour : la requête validée, qui garde une copie du JSON d'origine.
    /// # Errors
    ///
    /// `InvalidWrite` pour toute clé inconnue, identifiant, action, champ ou borne invalide ;
    /// `InvalidPatch` pour un patch de `update` invalide.
    pub fn parse(raw: &Value) -> Result<Self> {
        let map = only_keys(raw, &KEYS).ok_or(ErrorCode::InvalidWrite)?;
        let operation_id = map
            .get("operationId")
            .and_then(Value::as_str)
            .and_then(Uuid::parse)
            .ok_or(ErrorCode::InvalidWrite)?;
        let name = map
            .get("action")
            .and_then(Value::as_str)
            .ok_or(ErrorCode::InvalidWrite)?;
        let organization_id = actions::organization(map)?;
        let action = match name {
            "create" => WriteAction::Create {
                item: actions::create(map)?,
            },
            "create-folder" => WriteAction::CreateFolder {
                name: actions::create_folder(map)?,
            },
            "update" => {
                let (item_id, expected_revision, patch) = actions::update(map, organization_id.as_ref())?;
                WriteAction::Update {
                    item_id,
                    expected_revision,
                    patch,
                }
            }
            "delete" | "restore" => {
                let (item_id, expected_revision) = actions::item_lifecycle(map)?;
                if name == "delete" {
                    WriteAction::Delete {
                        item_id,
                        expected_revision,
                    }
                } else {
                    WriteAction::Restore {
                        item_id,
                        expected_revision,
                    }
                }
            }
            "update-folder" | "delete-folder" => match actions::folder_lifecycle(map, name == "update-folder")? {
                (folder_id, Some(name)) => WriteAction::UpdateFolder { folder_id, name },
                (folder_id, None) => WriteAction::DeleteFolder { folder_id },
            },
            _ => return Err(ErrorCode::InvalidWrite),
        };
        Ok(Self {
            operation_id,
            organization_id,
            action,
            raw: raw.clone(),
        })
    }

    /// Identifiant d'opération.
    #[must_use]
    pub fn operation_id(&self) -> &Uuid {
        &self.operation_id
    }

    /// Organisation de premier niveau de la requête, si elle est donnée.
    #[must_use]
    pub fn organization_id(&self) -> Option<&Uuid> {
        self.organization_id.as_ref()
    }

    /// Action validée.
    #[must_use]
    pub fn action(&self) -> &WriteAction {
        &self.action
    }

    /// Requête JSON d'origine, base de l'empreinte d'idempotence.
    #[must_use]
    pub fn raw(&self) -> &Value {
        &self.raw
    }
}
