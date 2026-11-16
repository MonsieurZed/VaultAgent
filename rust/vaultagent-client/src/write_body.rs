//! Corps de la route `write` : structures `serde` identiques aux objets de `agent-tools.mjs`.

use serde::Serialize;
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::error::ClientError;
use crate::write_values::MAX_VALUES_BYTES;

/// Identifiants de connexion d'une fiche à créer.
#[derive(Serialize)]
pub(crate) struct LoginBody<'a> {
    #[serde(flatten)]
    pub(crate) base: &'a Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) username: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) password: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) totp: Option<&'a str>,
}

/// Fiche à créer.
#[derive(Serialize)]
pub(crate) struct CreateItem<'a> {
    #[serde(flatten)]
    pub(crate) meta: &'a Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) notes: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) login: Option<LoginBody<'a>>,
}

/// Nom d'un dossier.
#[derive(Serialize)]
pub(crate) struct FolderItem<'a> {
    pub(crate) name: &'a str,
}

/// Contenu du champ `item`.
#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum ItemBody<'a> {
    /// Création d'une fiche.
    Create(CreateItem<'a>),
    /// Création ou renommage d'un dossier.
    Folder(FolderItem<'a>),
}

/// Modification ciblée d'une fiche.
#[derive(Serialize)]
pub(crate) struct PatchBody<'a> {
    #[serde(flatten)]
    pub(crate) meta: &'a Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) username: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) password: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) totp: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) notes: Option<&'a str>,
}

/// Corps complet de `POST /api/agent/write`.
#[derive(Serialize)]
pub(crate) struct WriteBody<'a> {
    #[serde(rename = "vaultId", skip_serializing_if = "Option::is_none")]
    pub(crate) vault_id: Option<&'a str>,
    #[serde(rename = "operationId")]
    pub(crate) operation_id: &'a str,
    pub(crate) action: &'static str,
    #[serde(rename = "itemId", skip_serializing_if = "Option::is_none")]
    pub(crate) item_id: Option<&'a str>,
    #[serde(rename = "folderId", skip_serializing_if = "Option::is_none")]
    pub(crate) folder_id: Option<&'a str>,
    #[serde(rename = "expectedRevisionDate", skip_serializing_if = "Option::is_none")]
    pub(crate) expected_revision_date: Option<&'a str>,
    #[serde(rename = "organizationId", skip_serializing_if = "Option::is_none")]
    pub(crate) organization_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) item: Option<ItemBody<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) patch: Option<PatchBody<'a>>,
}

impl<'a> WriteBody<'a> {
    /// Corps sans champ facultatif.
    ///
    /// Paramètres : le coffre, l'identifiant d'opération, l'action du protocole.
    /// Retour : le corps, à compléter champ par champ.
    #[must_use]
    pub(crate) fn new(vault_id: Option<&'a str>, operation_id: &'a str, action: &'static str) -> Self {
        Self {
            vault_id,
            operation_id,
            action,
            item_id: None,
            folder_id: None,
            expected_revision_date: None,
            organization_id: None,
            item: None,
            patch: None,
        }
    }

    /// Encode le corps dans un tampon effacé à la destruction.
    ///
    /// Retour : les octets JSON, ou `Usage` si l'encodage échoue.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub(crate) fn encode(&self) -> Result<Zeroizing<Vec<u8>>, ClientError> {
        let mut bytes = Zeroizing::new(Vec::with_capacity(4 * MAX_VALUES_BYTES));
        serde_json::to_writer(&mut *bytes, self)
            .map_err(|_| ClientError::Usage("Request is not encodable.".to_owned()))?;
        Ok(bytes)
    }
}
