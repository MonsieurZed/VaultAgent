//! Arguments `clap` des commandes d'écriture : `create`, `update`, `delete`, `restore` et `folder`.

use clap::{Args, Subcommand};

use crate::help;

/// Création d'une fiche.
#[derive(Debug, Clone, Args)]
pub struct CreateArgs {
    /// Identifiant d'opération (UUID) choisi par l'appelant et gardé pour une reprise.
    #[arg(
        long,
        value_name = "UUID",
        help = "Operation id (UUID) chosen by the caller and kept for any retry."
    )]
    pub operation_id: String,
    /// Métadonnées JSON sans secret (`type`, `name`, `folderId`, `login.uris`, `organizationId`, `collectionIds`).
    #[arg(
        long,
        value_name = "JSON",
        help = "Metadata JSON without secrets (type, name, folderId, login.uris, organizationId, collectionIds)."
    )]
    pub metadata: String,
    /// Lit les valeurs secrètes (JSON) sur l'entrée standard.
    #[arg(long, help = "Read the secret values (JSON object) from stdin.")]
    pub values_stdin: bool,
}

/// Mise à jour d'une fiche.
#[derive(Debug, Clone, Args)]
pub struct UpdateArgs {
    /// Identifiant d'opération (UUID).
    #[arg(long, value_name = "UUID", help = "Operation id (UUID).")]
    pub operation_id: String,
    /// Identifiant (UUID) de la fiche.
    #[arg(help = "Item id (UUID).")]
    pub item_id: String,
    /// Date de révision attendue, relue dans `preview`.
    #[arg(long, value_name = "DATE", help = "Expected revision date, read from preview.")]
    pub expected_revision: String,
    /// Organisation de la fiche, le cas échéant.
    #[arg(long, value_name = "UUID", help = "Organization of the item, if any.")]
    pub organization_id: Option<String>,
    /// Métadonnées JSON sans secret (`name`, `folderId`, `addUris`, `removeUris`).
    #[arg(
        long,
        value_name = "JSON",
        help = "Metadata JSON without secrets (name, folderId, addUris, removeUris)."
    )]
    pub metadata: Option<String>,
    /// Lit les valeurs secrètes (JSON) sur l'entrée standard.
    #[arg(long, help = "Read the secret values (JSON object) from stdin.")]
    pub values_stdin: bool,
}

/// Mise à la corbeille ou restauration d'une fiche.
#[derive(Debug, Clone, Args)]
pub struct TargetArgs {
    /// Identifiant d'opération (UUID).
    #[arg(long, value_name = "UUID", help = "Operation id (UUID).")]
    pub operation_id: String,
    /// Identifiant (UUID) de la fiche.
    #[arg(help = "Item id (UUID).")]
    pub item_id: String,
    /// Date de révision attendue.
    #[arg(long, value_name = "DATE", help = "Expected revision date.")]
    pub expected_revision: String,
    /// Organisation de la fiche, le cas échéant.
    #[arg(long, value_name = "UUID", help = "Organization of the item, if any.")]
    pub organization_id: Option<String>,
}

/// Création d'un dossier.
#[derive(Debug, Clone, Args)]
pub struct FolderCreateArgs {
    /// Identifiant d'opération (UUID).
    #[arg(long, value_name = "UUID", help = "Operation id (UUID).")]
    pub operation_id: String,
    /// Nom du dossier.
    #[arg(help = "Folder name.")]
    pub name: String,
}

/// Renommage d'un dossier.
#[derive(Debug, Clone, Args)]
pub struct FolderRenameArgs {
    /// Identifiant d'opération (UUID).
    #[arg(long, value_name = "UUID", help = "Operation id (UUID).")]
    pub operation_id: String,
    /// Identifiant (UUID) du dossier.
    #[arg(help = "Folder id (UUID).")]
    pub folder_id: String,
    /// Nouveau nom.
    #[arg(help = "New folder name.")]
    pub name: String,
}

/// Suppression d'un dossier.
#[derive(Debug, Clone, Args)]
pub struct FolderDeleteArgs {
    /// Identifiant d'opération (UUID).
    #[arg(long, value_name = "UUID", help = "Operation id (UUID).")]
    pub operation_id: String,
    /// Identifiant (UUID) du dossier.
    #[arg(help = "Folder id (UUID).")]
    pub folder_id: String,
}

/// Sous-commandes de `folder`.
#[derive(Debug, Clone, Subcommand)]
pub enum FolderAction {
    /// Crée un dossier.
    #[command(about = "Create a folder.", after_help = help::FOLDER_CREATE)]
    Create(FolderCreateArgs),
    /// Renomme un dossier.
    #[command(about = "Rename a folder.", after_help = help::FOLDER_RENAME)]
    Rename(FolderRenameArgs),
    /// Supprime un dossier.
    #[command(about = "Delete a folder.", after_help = help::FOLDER_DELETE)]
    Delete(FolderDeleteArgs),
}
