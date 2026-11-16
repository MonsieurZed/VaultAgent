//! Journal du faux moteur : un appel du trait `Engine` par valeur, identifiants en texte.
//!
//! Le mot de passe et le code sont des [`Secret`] : `Debug` n'en écrit pas le contenu, mais l'égalité les compare.

use serde_json::Value;
use vaultagent_core::NewItem;

use crate::passkey::Passkey;
use crate::secret::Secret;

/// Appel reçu par le faux moteur.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    /// `connect`.
    Connect {
        /// Origine du serveur.
        server: String,
        /// Courriel du compte.
        email: String,
        /// Mot de passe maître.
        password: Secret,
        /// Code de second facteur.
        code: Option<Secret>,
    },
    /// `lock`.
    Lock,
    /// `sync`.
    Sync,
    /// `items`.
    Items {
        /// Texte recherché.
        search: Option<String>,
    },
    /// `trashed_items`.
    TrashedItems,
    /// `item`.
    Item {
        /// Identifiant de la fiche.
        id: String,
    },
    /// `totp`.
    Totp {
        /// Identifiant de la fiche.
        id: String,
    },
    /// `folders`.
    Folders,
    /// `create_folder`.
    CreateFolder {
        /// Nom du dossier.
        name: String,
    },
    /// `rename_folder`.
    RenameFolder {
        /// Identifiant du dossier.
        id: String,
        /// Nouveau nom.
        name: String,
    },
    /// `delete_folder`.
    DeleteFolder {
        /// Identifiant du dossier.
        id: String,
    },
    /// `create_item`.
    CreateItem(Box<NewItem>),
    /// `update_item`.
    UpdateItem {
        /// Identifiant de la fiche.
        id: String,
        /// Révision attendue.
        expected_revision: String,
        /// Patch d'écriture.
        patch: Value,
        /// Organisation de la fiche.
        organization_id: Option<String>,
    },
    /// `trash_item`.
    TrashItem {
        /// Identifiant de la fiche.
        id: String,
    },
    /// `restore_item`.
    RestoreItem {
        /// Identifiant de la fiche.
        id: String,
    },
    /// `organizations`.
    Organizations,
    /// `check_sharing`.
    CheckSharing {
        /// Organisation visée.
        organization_id: String,
        /// Collections visées.
        collection_ids: Vec<String>,
    },
    /// `create_collection`.
    CreateCollection {
        /// Organisation visée.
        organization_id: String,
        /// Nom de la collection.
        name: String,
    },
    /// `share_item`.
    ShareItem {
        /// Identifiant de la fiche.
        id: String,
        /// Organisation de destination.
        organization_id: String,
        /// Collections de destination.
        collection_ids: Vec<String>,
    },
    /// `passkeys`.
    Passkeys {
        /// Domaine de la partie de confiance.
        rp_id: Option<String>,
    },
    /// `create_passkey`.
    CreatePasskey(Box<Passkey>),
}

impl Call {
    /// Nom de la méthode appelée, pour comparer des suites d'appels.
    ///
    /// Paramètres : aucun.
    /// Retour : le nom du trait `Engine`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Connect { .. } => "connect",
            Self::Lock => "lock",
            Self::Sync => "sync",
            Self::Items { .. } => "items",
            Self::TrashedItems => "trashed_items",
            Self::Item { .. } => "item",
            Self::Totp { .. } => "totp",
            Self::Folders => "folders",
            Self::CreateFolder { .. } => "create_folder",
            Self::RenameFolder { .. } => "rename_folder",
            Self::DeleteFolder { .. } => "delete_folder",
            Self::CreateItem(_) => "create_item",
            Self::UpdateItem { .. } => "update_item",
            Self::TrashItem { .. } => "trash_item",
            Self::RestoreItem { .. } => "restore_item",
            Self::Organizations => "organizations",
            Self::CheckSharing { .. } => "check_sharing",
            Self::CreateCollection { .. } => "create_collection",
            Self::ShareItem { .. } => "share_item",
            Self::Passkeys { .. } => "passkeys",
            Self::CreatePasskey(_) => "create_passkey",
        }
    }
}
