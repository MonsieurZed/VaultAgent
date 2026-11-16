//! Définition de la ligne de commande `vaultagent` (sous-commandes et options globales).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::help;
use crate::reads::ReadCommand;
use crate::seal::SealArgs;
use crate::write::WriteCommand;
use crate::write_args::{CreateArgs, FolderAction, TargetArgs, UpdateArgs};

/// Client `VaultAgent` : association et lecture du coffre, sorties JSON sur une ligne.
#[derive(Debug, Parser)]
#[command(
    name = "vaultagent",
    version,
    about = "VaultAgent client: pair a device, read and write vault items, unlock and seal secrets.",
    after_help = help::EXIT_CODES
)]
pub struct Cli {
    /// Fichier de configuration `{"server","token"}` (obligatoire sous Windows).
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "Configuration file {\"server\",\"token\"} (default: ~/.config/vaultagent/client.json)."
    )]
    pub config: Option<PathBuf>,
    /// Coffre visé, quand le jeton en autorise plusieurs.
    #[arg(
        long,
        global = true,
        value_name = "ID",
        help = "Vault to use when the token allows several."
    )]
    pub vault: Option<String>,
    /// Commande à exécuter.
    #[command(subcommand)]
    pub command: Command,
}

/// Sous-commandes disponibles.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Demande une association : affiche un code de vérification à faire approuver sur le serveur.
    #[command(about = "Request pairing; shows a verification code to approve on the server.", after_help = help::PAIR_REQUEST)]
    PairRequest {
        /// Adresse HTTPS du serveur, sans chemin.
        #[arg(help = "HTTPS address of the server, without path.")]
        server: String,
        /// Nom de l'appareil, par défaut celui de la machine.
        #[arg(help = "Device name (default: the machine name).")]
        name: Option<String>,
    },
    /// État du coffre et de la demande de déverrouillage.
    #[command(about = "Vault state and pending unlock request.", after_help = help::STATUS)]
    Status,
    /// Coffres autorisés pour ce jeton.
    #[command(about = "Vaults this token may use.", after_help = help::VAULTS)]
    Vaults,
    /// Organisations et collections accessibles.
    #[command(about = "Organizations and collections the vault can share with.", after_help = help::SHARING)]
    Sharing,
    /// Catalogue des fiches, sans secret.
    #[command(about = "Item catalog without secrets.", after_help = help::CATALOG)]
    Catalog {
        /// Texte recherché dans les noms.
        #[arg(help = "Text to look for in item names.")]
        search: Option<String>,
    },
    /// Dossiers et fiches, ou détail d'une fiche.
    #[command(about = "Folders and items, or the detail of one item.", after_help = help::PREVIEW)]
    Preview {
        /// Identifiant (UUID) de la fiche.
        #[arg(help = "Item id (UUID).")]
        id: Option<String>,
    },
    /// Crée une fiche (valeurs secrètes par l'entrée standard).
    #[command(about = "Create an item (secret values on stdin).", after_help = help::CREATE)]
    Create(CreateArgs),
    /// Modifie une fiche (valeurs secrètes par l'entrée standard).
    #[command(about = "Update an item (secret values on stdin).", after_help = help::UPDATE)]
    Update(UpdateArgs),
    /// Met une fiche à la corbeille.
    #[command(about = "Move an item to the trash.", after_help = help::DELETE)]
    Delete(TargetArgs),
    /// Restaure une fiche de la corbeille.
    #[command(about = "Restore an item from the trash.", after_help = help::RESTORE)]
    Restore(TargetArgs),
    /// Crée, renomme ou supprime un dossier.
    #[command(about = "Create, rename or delete a folder.", after_help = help::FOLDER)]
    Folder {
        /// Action sur le dossier.
        #[command(subcommand)]
        action: FolderAction,
    },
    /// Demande le déverrouillage du coffre, avec une attente facultative de l'approbation.
    #[command(about = "Request the vault unlock, optionally waiting for the approval.", after_help = help::UNLOCK)]
    Unlock {
        /// Attente en secondes (0 à 300).
        #[arg(
            long,
            value_name = "SECONDS",
            default_value_t = 0,
            allow_negative_numbers = true,
            help = "Seconds to wait for the approval (0 to 300)."
        )]
        wait: i64,
    },
    /// Verrouille le coffre.
    #[command(about = "Lock the vault.", after_help = help::LOCK)]
    Lock,
    /// Rend une enveloppe chiffrée pour une clé publique ; aucune valeur en clair ne sort.
    #[command(about = "Return a field as an envelope encrypted for a public key; no clear value is printed.", after_help = help::SEAL)]
    Seal(SealArgs),
}

impl Command {
    /// Commande de lecture équivalente, si ce n'est pas une association.
    ///
    /// Retour : la lecture, ou `None` pour `pair-request`.
    #[must_use]
    pub fn read(&self) -> Option<ReadCommand> {
        match self {
            Self::PairRequest { .. }
            | Self::Create(_)
            | Self::Update(_)
            | Self::Delete(_)
            | Self::Restore(_)
            | Self::Folder { .. }
            | Self::Unlock { .. }
            | Self::Lock
            | Self::Seal(_) => None,
            Self::Status => Some(ReadCommand::Status),
            Self::Vaults => Some(ReadCommand::Vaults),
            Self::Sharing => Some(ReadCommand::Sharing),
            Self::Catalog { search } => Some(ReadCommand::Catalog(search.clone())),
            Self::Preview { id } => Some(ReadCommand::Preview(id.clone())),
        }
    }
}

impl Command {
    /// Commande d'écriture équivalente, si c'en est une.
    ///
    /// Retour : l'écriture, ou `None` pour les autres commandes.
    #[must_use]
    pub fn write(&self) -> Option<WriteCommand> {
        match self {
            Self::Create(args) => Some(WriteCommand::Create(args.clone())),
            Self::Update(args) => Some(WriteCommand::Update(args.clone())),
            Self::Delete(args) => Some(WriteCommand::Delete(args.clone())),
            Self::Restore(args) => Some(WriteCommand::Restore(args.clone())),
            Self::Folder { action } => Some(match action {
                FolderAction::Create(args) => WriteCommand::FolderCreate(args.clone()),
                FolderAction::Rename(args) => WriteCommand::FolderRename(args.clone()),
                FolderAction::Delete(args) => WriteCommand::FolderDelete(args.clone()),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Analyse une ligne de commande de test.
    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("vaultagent").chain(args.iter().copied()))
    }

    #[test]
    fn global_options_work_before_and_after_the_command() {
        let cli = parse(&["--vault", "a", "status", "--config", "c.json"]).unwrap();
        assert_eq!(cli.vault.as_deref(), Some("a"));
        assert_eq!(cli.config, Some(PathBuf::from("c.json")));
        assert_eq!(cli.command.read(), Some(ReadCommand::Status));
    }

    #[test]
    fn arguments_reach_the_commands() {
        let cli = parse(&["catalog", "mail"]).unwrap();
        assert_eq!(cli.command.read(), Some(ReadCommand::Catalog(Some("mail".to_owned()))));
        let cli = parse(&["preview"]).unwrap();
        assert_eq!(cli.command.read(), Some(ReadCommand::Preview(None)));
        let cli = parse(&["pair-request", "https://x.test", "Box"]).unwrap();
        assert!(cli.command.read().is_none());
    }

    #[test]
    fn write_commands_are_recognised() {
        let id = "--operation-id=0000000a-0000-4000-8000-00000000000c";
        for args in [
            vec!["create", id, "--metadata", "{}"],
            vec!["update", "i", id, "--expected-revision", "d", "--values-stdin"],
            vec!["delete", "i", id, "--expected-revision", "d"],
            vec!["restore", "i", id, "--expected-revision", "d"],
            vec!["folder", "create", "n", id],
            vec!["folder", "rename", "f", "n", id],
            vec!["folder", "delete", "f", id],
        ] {
            assert!(parse(&args).unwrap().command.write().is_some(), "{args:?}");
        }
        assert!(parse(&["create", "--metadata", "{}"]).is_err());
    }

    #[test]
    fn unlock_wait_defaults_to_zero_and_lock_parses() {
        assert!(matches!(
            parse(&["unlock"]).unwrap().command,
            Command::Unlock { wait: 0 }
        ));
        assert!(matches!(
            parse(&["unlock", "--wait", "-3"]).unwrap().command,
            Command::Unlock { wait: -3 }
        ));
        assert!(matches!(parse(&["lock"]).unwrap().command, Command::Lock));
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert!(parse(&["steal"]).is_err());
        assert!(parse(&[]).is_err());
    }
}
