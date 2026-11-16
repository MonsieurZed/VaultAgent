//! Commandes d'écriture : validation des arguments, construction du corps `write`, appel et gestion du résultat indéterminé.

use std::io::Read;

use vaultagent_core::Uuid;

use crate::api::agent_call_bytes;
use crate::error::ClientError;
use crate::token::Token;
use crate::transport::Transport;
use crate::write_args::{CreateArgs, FolderCreateArgs, FolderDeleteArgs, FolderRenameArgs, TargetArgs, UpdateArgs};
use crate::write_body::{CreateItem, FolderItem, ItemBody, LoginBody, PatchBody, WriteBody};
use crate::write_metadata::{Metadata, parse_create, parse_update};
use crate::write_values::SecretValues;

/// Commande d'écriture, sans état : l'identifiant d'opération vient toujours de l'appelant.
#[derive(Debug, Clone)]
pub enum WriteCommand {
    /// `create`.
    Create(CreateArgs),
    /// `update`.
    Update(UpdateArgs),
    /// `delete` (corbeille).
    Delete(TargetArgs),
    /// `restore`.
    Restore(TargetArgs),
    /// `folder create`.
    FolderCreate(FolderCreateArgs),
    /// `folder rename`.
    FolderRename(FolderRenameArgs),
    /// `folder delete`.
    FolderDelete(FolderDeleteArgs),
}

/// Exige un UUID.
fn uuid<'a>(text: &'a str, what: &str) -> Result<&'a str, ClientError> {
    Uuid::parse(text)
        .map(|_| text)
        .ok_or_else(|| ClientError::Usage(format!("{what} must be a UUID.")))
}

/// Exige un texte non vide.
fn non_empty<'a>(text: &'a str, what: &str) -> Result<&'a str, ClientError> {
    if text.trim().is_empty() {
        return Err(ClientError::Usage(format!("{what} required.")));
    }
    Ok(text)
}

/// Lit les valeurs secrètes si `--values-stdin` est donné.
fn values(flag: bool, input: &mut dyn Read) -> Result<SecretValues, ClientError> {
    if flag {
        SecretValues::read(input)
    } else {
        Ok(SecretValues::default())
    }
}

/// Valide la cible d'une fiche et remplit les champs communs du corps.
fn target<'a>(body: &mut WriteBody<'a>, args: &'a TargetArgs) -> Result<(), ClientError> {
    body.item_id = Some(uuid(&args.item_id, "Item id")?);
    body.expected_revision_date = Some(non_empty(&args.expected_revision, "Expected revision")?);
    body.organization_id = args
        .organization_id
        .as_deref()
        .map(|id| uuid(id, "Organization id"))
        .transpose()?;
    Ok(())
}

/// Envoie un corps et traduit une coupure après envoi en erreur d'écriture indéterminée.
fn send(transport: &dyn Transport, token: &Token, body: &WriteBody<'_>) -> Result<String, ClientError> {
    let bytes = body.encode()?;
    match agent_call_bytes(transport, token, "write", &bytes) {
        Err(ClientError::Indeterminate) => Err(ClientError::WriteIndeterminate),
        other => other,
    }
}

/// Corps d'une création de fiche.
fn create(
    transport: &dyn Transport,
    token: &Token,
    vault: Option<&str>,
    args: &CreateArgs,
    input: &mut dyn Read,
) -> Result<String, ClientError> {
    let operation = uuid(&args.operation_id, "Operation id")?;
    let meta: Metadata = parse_create(&args.metadata)?;
    let secrets = values(args.values_stdin, input)?;
    let login_type = meta.is_login_type();
    if !login_type && secrets.has_login_field() {
        return Err(ClientError::Usage(
            "Only notes are allowed for this item type.".to_owned(),
        ));
    }
    let empty = serde_json::Map::new();
    let login = if login_type || meta.login.is_some() {
        Some(LoginBody {
            base: meta.login.as_ref().unwrap_or(&empty),
            username: secrets.username().filter(|_| login_type),
            password: secrets.password().filter(|_| login_type),
            totp: secrets.totp().filter(|_| login_type),
        })
    } else {
        None
    };
    let mut body = WriteBody::new(vault, operation, "create");
    body.item = Some(ItemBody::Create(CreateItem {
        meta: &meta.rest,
        notes: secrets.notes(),
        login,
    }));
    send(transport, token, &body)
}

/// Corps d'une mise à jour de fiche.
fn update(
    transport: &dyn Transport,
    token: &Token,
    vault: Option<&str>,
    args: &UpdateArgs,
    input: &mut dyn Read,
) -> Result<String, ClientError> {
    let operation = uuid(&args.operation_id, "Operation id")?;
    let meta = args
        .metadata
        .as_deref()
        .map(parse_update)
        .transpose()?
        .unwrap_or_default();
    let secrets = values(args.values_stdin, input)?;
    if meta.rest.is_empty() && !args.values_stdin {
        return Err(ClientError::Usage("Nothing to update.".to_owned()));
    }
    let mut body = WriteBody::new(vault, operation, "update");
    body.item_id = Some(uuid(&args.item_id, "Item id")?);
    body.expected_revision_date = Some(non_empty(&args.expected_revision, "Expected revision")?);
    body.organization_id = args
        .organization_id
        .as_deref()
        .map(|id| uuid(id, "Organization id"))
        .transpose()?;
    body.patch = Some(PatchBody {
        meta: &meta.rest,
        username: secrets.username(),
        password: secrets.password(),
        totp: secrets.totp(),
        notes: secrets.notes(),
    });
    send(transport, token, &body)
}

/// Corps d'une action sur dossier.
fn folder(
    transport: &dyn Transport,
    token: &Token,
    vault: Option<&str>,
    operation: &str,
    action: &'static str,
    folder_id: Option<&str>,
    name: Option<&str>,
) -> Result<String, ClientError> {
    let mut body = WriteBody::new(vault, uuid(operation, "Operation id")?, action);
    body.folder_id = folder_id.map(|id| uuid(id, "Folder id")).transpose()?;
    body.item = name
        .map(|text| non_empty(text, "Folder name"))
        .transpose()?
        .map(|name| ItemBody::Folder(FolderItem { name }));
    send(transport, token, &body)
}

impl WriteCommand {
    /// Exécute l'écriture.
    ///
    /// Paramètres : le transport, le jeton, le coffre choisi, l'entrée standard (lue seulement avec
    /// `--values-stdin`).
    /// Retour : le reçu du serveur sur une ligne, `Usage` pour un argument, une métadonnée ou une valeur invalide
    /// (rien n'est envoyé), `WriteIndeterminate` après un délai ou une coupure, sinon l'erreur classée.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn run(
        &self,
        transport: &dyn Transport,
        token: &Token,
        vault: Option<&str>,
        input: &mut dyn Read,
    ) -> Result<String, ClientError> {
        match self {
            Self::Create(args) => create(transport, token, vault, args, input),
            Self::Update(args) => update(transport, token, vault, args, input),
            Self::Delete(args) => item_action(transport, token, vault, args, "delete"),
            Self::Restore(args) => item_action(transport, token, vault, args, "restore"),
            Self::FolderCreate(a) => folder(
                transport,
                token,
                vault,
                &a.operation_id,
                "create-folder",
                None,
                Some(&a.name),
            ),
            Self::FolderRename(a) => folder(
                transport,
                token,
                vault,
                &a.operation_id,
                "update-folder",
                Some(&a.folder_id),
                Some(&a.name),
            ),
            Self::FolderDelete(a) => folder(
                transport,
                token,
                vault,
                &a.operation_id,
                "delete-folder",
                Some(&a.folder_id),
                None,
            ),
        }
    }
}

/// Corps d'une suppression ou d'une restauration de fiche.
fn item_action(
    transport: &dyn Transport,
    token: &Token,
    vault: Option<&str>,
    args: &TargetArgs,
    action: &'static str,
) -> Result<String, ClientError> {
    let mut body = WriteBody::new(vault, uuid(&args.operation_id, "Operation id")?, action);
    target(&mut body, args)?;
    send(transport, token, &body)
}
