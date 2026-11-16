//! Binaire `vaultagent` : lit la ligne de commande, branche le transport HTTPS réel et rend le code de sortie.

#![forbid(unsafe_code)]

use std::io::Write;

use clap::Parser;
use clap::error::ErrorKind;
use vaultagent_client::cli::Cli;
use vaultagent_client::error::ClientError;
use vaultagent_client::https_transport::HttpsTransport;
use vaultagent_client::machine_name::machine_name;
use vaultagent_client::pause::ThreadPause;
use vaultagent_client::run::{Runtime, run};
use vaultagent_client::transport::Transport;

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
    let mut pause = ThreadPause;
    let factory = |address: &vaultagent_client::server_address::ServerAddress| -> Box<dyn Transport> {
        Box::new(HttpsTransport::new(address.clone()))
    };
    let mut runtime = Runtime {
        home: std::env::var_os("HOME"),
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
