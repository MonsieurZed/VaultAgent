//! Aides communes des tests d'intégration : dossier temporaire, transport partagé, exécution de `run`.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Parser;
use vaultagent_client::cli::Cli;
use vaultagent_client::exit_code::ExitCode;
use vaultagent_client::fake_transport::FakeTransport;
use vaultagent_client::pause::RecordedPause;
use vaultagent_client::run::{Runtime, run};
use vaultagent_client::server_address::ServerAddress;
use vaultagent_client::transport::{Response, Transport, TransportError};

/// Jeton factice de la forme du protocole.
/// Jeton factice de la forme du protocole, construit à l'exécution.
pub fn token() -> String {
    format!("va1.0000000a-0000-4000-8000-00000000000b.{}", "A".repeat(43))
}

/// Contenu d'une configuration valide.
pub fn config_text() -> String {
    format!(
        "{{\"server\":\"https://vault.example.test\",\"token\":\"{}\"}}",
        token()
    )
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Dossier temporaire supprimé à la destruction.
pub struct TempDir(PathBuf);

impl TempDir {
    /// Crée un dossier au nom unique.
    pub fn new() -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("vaultagent-it-{}-{nanos}-{count}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    /// Chemin du dossier.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Écrit une configuration privée valide et rend son chemin.
    pub fn write_config(&self) -> PathBuf {
        let path = self.0.join("client.json");
        vaultagent_client::private_file::write_private_atomic(&path, config_text().as_bytes()).unwrap();
        path
    }
}

impl Drop for TempDir {
    /// Supprime le dossier.
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap_or_default();
    }
}

/// Transport partageable entre plusieurs appels de la fabrique.
pub struct Shared(pub Rc<FakeTransport>);

impl Transport for Shared {
    /// Délègue au faux serveur partagé.
    fn post(
        &self,
        path: &str,
        bearer: Option<&str>,
        body: &[u8],
        timeout: Duration,
    ) -> Result<Response, TransportError> {
        self.0.post(path, bearer, body, timeout)
    }
}

/// Résultat d'une exécution de `run`.
pub struct Outcome {
    /// Code de sortie.
    pub exit: ExitCode,
    /// Contenu de stdout.
    pub out: String,
    /// Contenu de stderr.
    pub err: String,
    /// Attentes demandées.
    pub pauses: usize,
}

/// Exécute `vaultagent <args>` contre le faux serveur, sans réseau ni attente réelle.
pub fn execute(fake: &Rc<FakeTransport>, args: &[&str]) -> Outcome {
    execute_with_input(fake, args, b"")
}

/// Exécute `vaultagent <args>` avec un contenu d'entrée standard donné.
pub fn execute_with_input(fake: &Rc<FakeTransport>, args: &[&str], input: &[u8]) -> Outcome {
    let cli = Cli::try_parse_from(std::iter::once("vaultagent").chain(args.iter().copied())).unwrap();
    let shared = Rc::clone(fake);
    let factory = move |_address: &ServerAddress| -> Box<dyn Transport> { Box::new(Shared(Rc::clone(&shared))) };
    let mut pause = RecordedPause::default();
    let mut runtime = Runtime {
        home: None,
        machine_name: "test-machine".to_owned(),
        transports: &factory,
        pause: &mut pause,
        input: &mut &input[..],
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = run(&cli, &mut runtime, &mut out, &mut err);
    Outcome {
        exit,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
        pauses: pause.durations.len(),
    }
}
