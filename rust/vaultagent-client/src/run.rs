//! Exécution d'une commande : configuration, transport, association ou lecture, sorties et code de sortie.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::approval_commands::{Environment, run_approval, run_sign};
use crate::cli::{Cli, Command};
use crate::config_store::ConfigStore;
use crate::elevation_system::ElevationSystem;
use crate::error::ClientError;
use crate::exit_code::ExitCode;
use crate::hello_system::HelloSystem;
use crate::pairing_flow::pair_and_save;
use crate::pairing_wire::PairedOutput;
use crate::pause::Pause;
use crate::seal::run_seal;
use crate::server_address::ServerAddress;
use crate::transport::Transport;
use crate::unlock::{Outcome, lock, unlock};
use crate::window_commands;
use crate::window_system::system;

/// Fabrique du transport vers un serveur.
pub type TransportFactory<'a> = &'a dyn Fn(&ServerAddress) -> Box<dyn Transport>;

/// Environnement injecté : dossier personnel, nom de machine, transport et attente.
pub struct Runtime<'a> {
    /// Dossier personnel (`HOME`).
    pub home: Option<OsString>,
    /// Dossier de données de l'application (`%LOCALAPPDATA%\VaultAgent` sous Windows), `None` ailleurs.
    pub local_data: Option<PathBuf>,
    /// Dossier des données communes (`%ProgramData%` sous Windows), `None` ailleurs.
    pub program_data: Option<PathBuf>,
    /// Dossier temporaire de l'utilisateur.
    pub temp_dir: PathBuf,
    /// Système d'élévation (jeton, processus élevé, DPAPI machine).
    pub elevation: &'a dyn ElevationSystem,
    /// Système Windows Hello (disponibilité, enregistrement, signature).
    pub hello: &'a dyn HelloSystem,
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
    options: (Option<&str>, bool),
    err: &mut dyn Write,
) -> Result<Outcome, ClientError> {
    let (name, approver) = options;
    let server = ServerAddress::parse(address)?;
    let store = locate(cli, runtime)?;
    let transport = (runtime.transports)(&server);
    let device = name
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(&runtime.machine_name)
        .to_owned();
    let vault_id = pair_and_save(
        &store,
        transport.as_ref(),
        server,
        &device,
        approver,
        runtime.pause,
        err,
    )?;
    let output = PairedOutput { paired: true, vault_id };
    let line =
        serde_json::to_string(&output).map_err(|_| ClientError::BadResponse("Output not encodable.".to_owned()))?;
    Ok(success(line))
}

/// Détermine le stockage de la configuration.
fn locate(cli: &Cli, runtime: &Runtime<'_>) -> Result<ConfigStore, ClientError> {
    ConfigStore::locate(
        cli.config.as_deref(),
        runtime.home.as_deref(),
        runtime.local_data.as_deref(),
    )
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
        Command::PairRequest { server, name, approver } => {
            pair(cli, runtime, server, (name.as_deref(), *approver), err)
        }
        Command::Foreground => window_commands::foreground(system()?.as_ref()),
        Command::Tray => Err(ClientError::Usage(
            "The tray app starts only from the vaultagent binary on Windows.".to_owned(),
        )),
        Command::Focus(args) => window_commands::focus(system()?.as_ref(), args),
        Command::ElevationSign(args) => run_sign(runtime.elevation, runtime.program_data.as_deref(), args),
        other => {
            let store = locate(cli, runtime)?;
            let config = store.load()?;
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
                Command::Approval(approval) => {
                    let environment = Environment {
                        store: &store,
                        system: runtime.elevation,
                        hello: runtime.hello,
                        temp_dir: &runtime.temp_dir,
                    };
                    run_approval(approval, transport.as_ref(), &config, vault, &environment)
                }
                Command::Type(args) => {
                    window_commands::type_text(system()?.as_ref(), transport.as_ref(), &config, vault, args)
                }
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
            if !outcome.line.is_empty() {
                writeln!(out, "{}", outcome.line).unwrap_or_default();
            }
            outcome.exit
        }
        Err(error) => {
            writeln!(err, "{}", error.error_line()).unwrap_or_default();
            error.exit_code()
        }
    }
}
