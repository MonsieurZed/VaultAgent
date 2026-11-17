//! Commandes d'approbation : `approvals` (demandes en attente), `decide`, `elevation-enroll`, `hello-status`,
//! `hello-enroll` et la commande cachée `elevation-sign`, exécutée par le processus élevé.

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::approval_request::fetch;
use crate::approver_key::ApproverKey;
use crate::config::ClientConfig;
use crate::config_store::ConfigStore;
use crate::decide::Approver;
use crate::elevation_sign::run_elevation_sign;
use crate::elevation_system::ElevationSystem;
use crate::error::ClientError;
use crate::exit_code::ExitCode;
use crate::hello_enroll::hello_status;
use crate::hello_system::HelloSystem;
use crate::proofs::SystemProofs;
use crate::transport::Transport;
use crate::unlock::Outcome;

/// Arguments de `decide`.
#[derive(Debug, Clone, Args)]
pub struct DecideArgs {
    /// Identifiant de la demande à décider.
    #[arg(value_name = "REQUEST_ID", help = "Id of the pending request (see `approvals`).")]
    pub request_id: String,
    /// Approuve la demande.
    #[arg(
        long,
        conflicts_with = "deny",
        required_unless_present = "deny",
        help = "Approve the request."
    )]
    pub approve: bool,
    /// Refuse la demande.
    #[arg(long, help = "Deny the request.")]
    pub deny: bool,
    /// Mémorise l'acceptation pour les demandes suivantes de ce client.
    #[arg(long, conflicts_with = "deny", help = "Always accept requests from this client.")]
    pub remember: bool,
}

/// Arguments de la commande cachée `elevation-sign`.
#[derive(Debug, Clone, Args)]
pub struct ElevationSignArgs {
    /// SID de l'utilisateur dont la clé d'élévation signe.
    pub sid: String,
    /// Message à signer, en base64.
    pub message: String,
    /// Fichier qui reçoit la clé publique puis la signature.
    pub output: PathBuf,
    /// Dossier de base de la clé à la place de `%ProgramData%` (essais réels dans un dossier de test).
    #[arg(long, hide = true, value_name = "DIR")]
    pub program_data: Option<PathBuf>,
}

/// Commandes d'approbation qui parlent au serveur.
#[derive(Debug, Clone, Subcommand)]
pub enum ApprovalCommand {
    /// Liste les demandes de déverrouillage en attente.
    #[command(about = "List the pending unlock requests.", after_help = crate::help::APPROVALS)]
    Approvals,
    /// Approuve ou refuse une demande, avec les preuves exigées.
    #[command(about = "Approve or deny a pending request (Windows validation when required).", after_help = crate::help::DECIDE)]
    Decide(DecideArgs),
    /// Enrôle la clé d'élévation de ce poste sur les coffres qui n'en ont pas.
    #[command(about = "Enroll this PC's elevation key on vaults without one.", after_help = crate::help::ELEVATION_ENROLL)]
    ElevationEnroll,
    /// Indique si Windows Hello est disponible, sans invite.
    #[command(about = "Tell whether Windows Hello is available (no prompt).", after_help = crate::help::HELLO_STATUS)]
    HelloStatus,
    /// Enrôle Windows Hello sur les coffres où il n'est pas encore exigé.
    #[command(about = "Enroll Windows Hello on vaults that do not require it yet.", after_help = crate::help::HELLO_ENROLL)]
    HelloEnroll,
}

/// Environnement système des commandes d'approbation.
pub struct Environment<'a> {
    /// Stockage de la configuration (porte la clé d'approbateur).
    pub store: &'a ConfigStore,
    /// Système d'élévation.
    pub system: &'a dyn ElevationSystem,
    /// Système Windows Hello.
    pub hello: &'a dyn HelloSystem,
    /// Dossier temporaire de l'utilisateur.
    pub temp_dir: &'a Path,
}

/// Enveloppe une ligne de sortie réussie.
fn success(line: String) -> Outcome {
    Outcome {
        line,
        exit: ExitCode::Success,
    }
}

/// Demande en attente telle qu'affichée par `approvals`.
#[derive(Serialize)]
struct PendingView<'a> {
    #[serde(rename = "vaultId")]
    vault_id: &'a str,
    #[serde(rename = "requestId")]
    request_id: &'a str,
    challenge: &'a str,
    #[serde(rename = "helloRequired")]
    hello_required: bool,
    #[serde(rename = "elevationRequired")]
    elevation_required: bool,
}

/// Sortie de `approvals`.
#[derive(Serialize)]
struct PendingList<'a> {
    #[serde(rename = "approvalRequests")]
    approval_requests: Vec<PendingView<'a>>,
}

/// Liste les demandes en attente.
fn approvals(transport: &dyn Transport, config: &ClientConfig, vault: Option<&str>) -> Result<Outcome, ClientError> {
    let pending = fetch(transport, &config.token, vault)?;
    let requests = pending
        .iter()
        .map(|request| PendingView {
            vault_id: request.vault(),
            request_id: &request.request_id,
            challenge: &request.challenge,
            hello_required: request.hello_required,
            elevation_required: request.elevation_required,
        })
        .collect();
    serde_json::to_string(&PendingList {
        approval_requests: requests,
    })
    .map(success)
    .map_err(|_| ClientError::BadResponse("Output not encodable.".to_owned()))
}

/// Décide la demande nommée.
fn decide(
    approver: &Approver<'_>,
    vault: Option<&str>,
    args: &DecideArgs,
    environment: &Environment<'_>,
) -> Result<Outcome, ClientError> {
    let pending = fetch(approver.transport, &approver.config.token, vault)?;
    let request = pending
        .iter()
        .find(|request| request.request_id == args.request_id)
        .ok_or_else(|| ClientError::Server {
            status: 404,
            code: "request-not-found".to_owned(),
        })?;
    let proofs = SystemProofs::new(environment.system, environment.hello, environment.temp_dir);
    approver
        .decide_unlock(request, args.approve && !args.deny, args.remember, &proofs)
        .map(success)
}

/// Exécute une commande d'approbation.
///
/// Paramètres : la commande, le transport, la configuration, le coffre (`--vault`) et l'environnement.
/// Retour : la ligne de sortie ; `Usage` si la clé d'approbateur est introuvable (stockage `--config`), erreur
/// classée du serveur, ou erreur de preuve (UAC annulée, Hello indisponible).
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn run_approval(
    command: &ApprovalCommand,
    transport: &dyn Transport,
    config: &ClientConfig,
    vault: Option<&str>,
    environment: &Environment<'_>,
) -> Result<Outcome, ClientError> {
    if matches!(command, ApprovalCommand::Approvals) {
        return approvals(transport, config, vault);
    }
    if matches!(command, ApprovalCommand::HelloStatus) {
        return hello_status(environment.hello).map(success);
    }
    let key: ApproverKey = environment.store.approver_key()?;
    let approver = Approver {
        transport,
        config,
        key: &key,
    };
    if let ApprovalCommand::Decide(args) = command {
        return decide(&approver, vault, args, environment);
    }
    if matches!(command, ApprovalCommand::HelloEnroll) {
        return approver.enroll_hello(environment.hello, vault).map(success);
    }
    let proofs = SystemProofs::new(environment.system, environment.hello, environment.temp_dir);
    approver.enroll_elevation(&proofs).map(success)
}

/// Exécute la commande cachée `elevation-sign` dans le processus élevé.
///
/// Paramètres : le système, le dossier `%ProgramData%`, les arguments.
/// Retour : une sortie vide ; `ElevationRequired` (code 2) sans élévation.
///
/// # Errors
///
/// Les erreurs possibles sont celles de `run_elevation_sign`.
pub fn run_sign(
    system: &dyn ElevationSystem,
    program_data: Option<&Path>,
    args: &ElevationSignArgs,
) -> Result<Outcome, ClientError> {
    let base = args.program_data.as_deref().or(program_data);
    run_elevation_sign(system, base, &args.sid, &args.message, &args.output).map(|()| success(String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command};
    use clap::Parser;

    /// Analyse une ligne de commande de test.
    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("vaultagent").chain(args.iter().copied()))
    }

    #[test]
    fn approval_commands_parse() {
        assert!(matches!(
            parse(&["approvals"]).unwrap().command,
            Command::Approval(ApprovalCommand::Approvals)
        ));
        assert!(matches!(
            parse(&["elevation-enroll"]).unwrap().command,
            Command::Approval(ApprovalCommand::ElevationEnroll)
        ));
        assert!(matches!(
            parse(&["hello-status"]).unwrap().command,
            Command::Approval(ApprovalCommand::HelloStatus)
        ));
        assert!(matches!(
            parse(&["hello-enroll"]).unwrap().command,
            Command::Approval(ApprovalCommand::HelloEnroll)
        ));
        let decide = parse(&["decide", "r1", "--approve", "--remember"]).unwrap();
        assert!(matches!(
            decide.command,
            Command::Approval(ApprovalCommand::Decide(ref args))
                if args.request_id == "r1" && args.approve && args.remember && !args.deny
        ));
        let deny = parse(&["decide", "r1", "--deny"]).unwrap();
        assert!(matches!(deny.command, Command::Approval(ApprovalCommand::Decide(ref args)) if args.deny));
    }

    #[test]
    fn decide_needs_exactly_one_decision() {
        assert!(parse(&["decide", "r1"]).is_err());
        assert!(parse(&["decide", "r1", "--approve", "--deny"]).is_err());
        assert!(parse(&["decide", "r1", "--deny", "--remember"]).is_err());
    }

    #[test]
    fn elevation_sign_is_hidden_but_parses() {
        let cli = parse(&["elevation-sign", "S-1-5-21-1", "bQ==", "out.txt"]).unwrap();
        assert!(matches!(cli.command, Command::ElevationSign(ref args) if args.sid == "S-1-5-21-1"));
        assert!(parse(&["elevation-sign", "S-1"]).is_err());
        let help = <Cli as clap::CommandFactory>::command().render_help().to_string();
        assert!(!help.contains("elevation-sign") && help.contains("decide"));
    }
}
