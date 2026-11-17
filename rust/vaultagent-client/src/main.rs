//! Binaire `vaultagent` : lit la ligne de commande, branche le transport HTTPS réel et rend le code de sortie.

#![forbid(unsafe_code)]

use std::io::Write;

use clap::Parser;
use clap::error::ErrorKind;
use vaultagent_client::cli::{Cli, Command};
use vaultagent_client::elevation_system::system_elevation;
use vaultagent_client::error::ClientError;
use vaultagent_client::hello_system::system_hello;
use vaultagent_client::https_transport::HttpsTransport;
use vaultagent_client::machine_name::machine_name;
use vaultagent_client::pause::ThreadPause;
use vaultagent_client::run::{Runtime, run};
use vaultagent_client::transport::Transport;

/// Dossier de données de l'application : `%LOCALAPPDATA%\VaultAgent` sous Windows, absent ailleurs.
fn local_data() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|base| cfg!(windows) && !base.is_empty())
        .map(|base| std::path::PathBuf::from(base).join("VaultAgent"))
}

/// Dossier des données communes : `%ProgramData%` sous Windows, absent ailleurs.
fn program_data() -> Option<std::path::PathBuf> {
    std::env::var_os("ProgramData")
        .filter(|base| cfg!(windows) && !base.is_empty())
        .map(std::path::PathBuf::from)
}

/// Lance l'application de barre (Windows) ; refuse ailleurs.
///
/// Paramètres : la ligne de commande analysée.
/// Retour : le code de sortie, erreur JSON sur stderr si l'application ne démarre pas.
#[cfg(windows)]
fn run_tray(cli: &Cli) -> std::process::ExitCode {
    let setup = vaultagent_client::tray_worker::WorkerSetup {
        config: cli.config.clone(),
        home: std::env::var_os("HOME"),
        local_data: local_data(),
        temp_dir: std::env::temp_dir(),
        machine_name: machine_name(None, |name| std::env::var(name).ok()),
        language: vaultagent_client::translate::Language::English,
    };
    match vaultagent_client::tray_run::run_tray(setup) {
        Ok(code) => std::process::ExitCode::from(code.code()),
        Err(error) => {
            writeln!(std::io::stderr(), "{}", error.error_line()).unwrap_or_default();
            std::process::ExitCode::from(error.exit_code().code())
        }
    }
}

/// Point d'entrée : erreurs d'usage sur stderr en JSON (code 1), aide et version sur stdout (code 0).
fn main() -> std::process::ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            print!("{error}");
            return std::process::ExitCode::SUCCESS;
        }
        Err(error) => {
            let first = error.to_string().lines().next().unwrap_or("Invalid usage.").to_owned();
            let usage = ClientError::Usage(first);
            writeln!(std::io::stderr(), "{}", usage.error_line()).unwrap_or_default();
            return std::process::ExitCode::from(usage.exit_code().code());
        }
    };
    #[cfg(windows)]
    if matches!(cli.command, Command::Tray) {
        return run_tray(&cli);
    }
    let mut pause = ThreadPause;
    let factory = |address: &vaultagent_client::server_address::ServerAddress| -> Box<dyn Transport> {
        Box::new(HttpsTransport::new(address.clone()))
    };
    let elevation = system_elevation();
    let hello = system_hello();
    let mut runtime = Runtime {
        home: std::env::var_os("HOME"),
        local_data: local_data(),
        program_data: program_data(),
        temp_dir: std::env::temp_dir(),
        elevation: elevation.as_ref(),
        hello: hello.as_ref(),
        machine_name: machine_name(std::fs::read_to_string("/etc/hostname").ok(), |name| {
            std::env::var(name).ok()
        }),
        transports: &factory,
        pause: &mut pause,
        input: &mut std::io::stdin().lock(),
    };
    let code = run(&cli, &mut runtime, &mut std::io::stdout(), &mut std::io::stderr());
    std::process::ExitCode::from(code.code())
}
