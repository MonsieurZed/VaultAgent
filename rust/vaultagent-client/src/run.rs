//! Exécution d'une commande : configuration, transport, association ou lecture, sorties et code de sortie.

use std::ffi::OsString;
use std::io::{Read, Write};

use crate::cli::{Cli, Command};
use crate::config::ClientConfig;
use crate::config_path::resolve;
use crate::error::ClientError;
use crate::exit_code::ExitCode;
use crate::pairing::pair_request;
use crate::pairing_wire::PairedOutput;
use crate::pause::Pause;
use crate::seal::run_seal;
use crate::server_address::ServerAddress;
use crate::transport::Transport;
use crate::unlock::{Outcome, lock, unlock};

/// Fabrique du transport vers un serveur.
pub type TransportFactory<'a> = &'a dyn Fn(&ServerAddress) -> Box<dyn Transport>;

/// Environnement injecté : dossier personnel, nom de machine, transport et attente.
pub struct Runtime<'a> {
    /// Dossier personnel (`HOME`).
    pub home: Option<OsString>,
    /// Nom de la machine, utilisé par défaut pour `pair-request`.
    pub machine_name: String,
    /// Fabrique du transport.
    pub transports: TransportFactory<'a>,
    /// Attente entre deux interrogations.
    pub pause: &'a mut dyn Pause,
    /// Entrée standard, lue seulement par `--values-stdin`.
    pub input: &'a mut dyn Read,
}

/// Association : demande, attente de l'approbation et enregistrement de la configuration.
fn pair(
    cli: &Cli,
    runtime: &mut Runtime<'_>,
    address: &str,
    name: Option<&str>,
    err: &mut dyn Write,
) -> Result<Outcome, ClientError> {
    let server = ServerAddress::parse(address)?;
    let path = resolve(cli.config.as_deref(), runtime.home.as_deref())?;
    let transport = (runtime.transports)(&server);
    let device = name
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(&runtime.machine_name);
    let paired = pair_request(transport.as_ref(), &server, device, runtime.pause, err)?;
    let config = ClientConfig {
        server,
        token: paired.token,
    };
    config.save(&path)?;
    let output = PairedOutput {
        paired: true,
        vault_id: paired.vault_id,
    };
    let line =
        serde_json::to_string(&output).map_err(|_| ClientError::BadResponse("Output not encodable.".to_owned()))?;
    Ok(success(line))
}

/// Enveloppe une ligne de sortie réussie.
fn success(line: String) -> Outcome {
    Outcome {
        line,
        exit: ExitCode::Success,
    }
}

/// Exécute la commande demandée.
fn execute(cli: &Cli, runtime: &mut Runtime<'_>, err: &mut dyn Write) -> Result<Outcome, ClientError> {
    match &cli.command {
        Command::PairRequest { server, name } => pair(cli, runtime, server, name.as_deref(), err),
        other => {
            let path = resolve(cli.config.as_deref(), runtime.home.as_deref())?;
            let config = ClientConfig::load(&path)?;
            let transport = (runtime.transports)(&config.server);
            let (token, vault) = (&config.token, cli.vault.as_deref());
            if let Some(read) = other.read() {
                return read.run(transport.as_ref(), token, vault).map(success);
            }
            if let Some(write) = other.write() {
                return write.run(transport.as_ref(), token, vault, runtime.input).map(success);
            }
            match other {
                Command::Unlock { wait } => unlock(transport.as_ref(), token, vault, *wait, runtime.pause),
                Command::Lock => lock(transport.as_ref(), token, vault).map(success),
                Command::Seal(args) => run_seal(transport.as_ref(), token, vault, args).map(success),
                _ => Err(ClientError::Usage("Unsupported command.".to_owned())),
            }
        }
    }
}

/// Exécute une commande et écrit ses sorties.
///
/// Paramètres : la ligne de commande lue, l'environnement injecté, stdout, stderr.
/// Retour : le code de sortie ; stdout reçoit le corps JSON sur une ligne, stderr la ligne d'erreur JSON.
pub fn run(cli: &Cli, runtime: &mut Runtime<'_>, out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    match execute(cli, runtime, err) {
        Ok(outcome) => {
            writeln!(out, "{}", outcome.line).unwrap_or_default();
            outcome.exit
        }
        Err(error) => {
            writeln!(err, "{}", error.error_line()).unwrap_or_default();
            error.exit_code()
        }
    }
}
