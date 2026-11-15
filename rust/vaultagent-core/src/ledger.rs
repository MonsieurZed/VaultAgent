//! Types du registre d'idempotence (`write-receipts.json`, `share-receipts.json`) : entrées, reçus et accès au
//! stockage. Le format JSON est celui de Node, le serveur Rust reprend `/data` sans migration.

use crate::error::{ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Nombre maximal d'entrées d'un registre.
pub const CAPACITY: usize = 10_000;

/// Reçu d'une opération vérifiée. Aucun champ ne porte une valeur de fiche : seulement des identifiants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    /// Identifiant de la fiche, du dossier ou de la collection concernée.
    pub id: String,
    /// Identifiant d'opération de la requête.
    pub operation_id: String,
    /// Nom de l'action (`create`, `update`, `apply`, …).
    pub action: String,
    /// Effet constaté par relecture.
    pub verified: bool,
    /// Date de révision relue, pour les actions sur une fiche.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_date: Option<String>,
    /// Organisation, pour les reçus de partage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    /// Présent et vrai quand le reçu est rejoué.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replayed: Option<bool>,
}

impl Receipt {
    /// Construit un reçu vérifié.
    ///
    /// Paramètres : l'identifiant concerné, l'identifiant d'opération, l'action, la date de révision relue.
    /// Retour : le reçu, sans organisation ni marque de rejeu.
    #[must_use]
    pub fn verified(id: &str, operation_id: &str, action: &str, revision_date: Option<&str>) -> Self {
        Self {
            id: id.to_owned(),
            operation_id: operation_id.to_owned(),
            action: action.to_owned(),
            verified: true,
            revision_date: revision_date.map(str::to_owned),
            organization_id: None,
            replayed: None,
        }
    }
}

/// Entrée d'un registre : l'intention d'écrire (empreinte) puis, une fois l'effet vérifié, le reçu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Empreinte HMAC de la requête.
    pub fingerprint: String,
    /// Reçu, absent tant que l'écriture n'est pas vérifiée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
}

/// Contenu d'un registre : entrées par clé `<profil>:<operationId>`.
pub type Ledger = BTreeMap<String, LedgerEntry>;

/// Registre concerné : écritures ou partage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registry {
    /// `write-receipts.json`.
    Write,
    /// `share-receipts.json`.
    Share,
}

impl Registry {
    /// Nom du fichier du registre dans `/data`.
    #[must_use]
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Write => "write-receipts.json",
            Self::Share => "share-receipts.json",
        }
    }

    /// Code rendu quand une intention existe sans reçu.
    #[must_use]
    pub fn indeterminate(self) -> ErrorCode {
        match self {
            Self::Write => ErrorCode::WriteIndeterminate,
            Self::Share => ErrorCode::ShareIndeterminate,
        }
    }
}

/// Accès au stockage des registres ; l'implémentation fichier vient avec le serveur.
pub trait ReceiptStore {
    /// Charge un registre, vide s'il n'existe pas encore.
    ///
    /// # Errors
    ///
    /// Un code d'erreur du protocole si la lecture échoue.
    fn load(&self, registry: Registry) -> Result<Ledger>;

    /// Enregistre un registre en entier.
    ///
    /// # Errors
    ///
    /// Un code d'erreur du protocole si l'écriture échoue.
    fn save(&mut self, registry: Registry, ledger: &Ledger) -> Result<()>;
}
