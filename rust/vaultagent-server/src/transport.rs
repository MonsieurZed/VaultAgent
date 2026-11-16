//! Transport HTTP : `tiny_http` autour de [`route`], un ensemble fixe de fils de travail et la minuterie qui
//! verrouille les sessions échues (toutes les 500 ms comme Node).

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tiny_http::Server;

use crate::context::Context;
use crate::error::{Result, ServerError};
use crate::route::route;
use crate::transport_request::read;
use crate::transport_response::{bare, convert};

const POLL: Duration = Duration::from_millis(100);
const TICKS_PER_EXPIRY: u32 = 5;

/// Service en écoute ; ses fils s'arrêtent par [`Running::shutdown`].
pub struct Running {
    address: Option<SocketAddr>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl Running {
    /// Adresse d'écoute réelle (utile avec le port 0).
    ///
    /// Paramètres : aucun.
    /// Retour : l'adresse, ou `None` si l'écoute n'est pas sur un port TCP.
    #[must_use]
    pub fn address(&self) -> Option<SocketAddr> {
        self.address
    }

    /// Attend la fin des fils, c'est-à-dire un arrêt demandé par ailleurs ; sert au binaire.
    ///
    /// Paramètres : aucun.
    pub fn wait(self) {
        for thread in self.threads {
            if thread.join().is_err() {
                eprintln!("un fil du serveur s'est arrêté sur une erreur");
            }
        }
    }

    /// Arrête les fils de travail et la minuterie, puis attend leur fin (au plus quelques centaines de ms).
    ///
    /// Paramètres : aucun.
    pub fn shutdown(self) {
        self.stop.store(true, Ordering::SeqCst);
        self.wait();
    }
}

/// Ouvre le port et lance les fils.
///
/// Paramètres : l'adresse `hôte:port`, le nombre de fils de travail, le contexte des routes.
/// Retour : le service en écoute.
///
/// # Errors
///
/// `Listen` si le port ne s'ouvre pas, `Io` si un fil ne démarre pas.
pub fn serve(address: &str, workers: usize, context: &Context) -> Result<Running> {
    let server = Server::http(address).map_err(|error| ServerError::Listen(error.to_string()))?;
    let local = server.server_addr().to_ip();
    let server = Arc::new(server);
    let stop = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();
    for index in 0..workers {
        let (server, stop, context) = (Arc::clone(&server), Arc::clone(&stop), context.clone());
        threads.push(
            thread::Builder::new()
                .name(format!("vaultagent-worker-{index}"))
                .spawn(move || work(&server, &stop, &context))?,
        );
    }
    let (stop_ticker, vaults) = (Arc::clone(&stop), Arc::clone(&context.vaults));
    threads.push(
        thread::Builder::new()
            .name("vaultagent-expiry".to_owned())
            .spawn(move || {
                let mut ticks = 0;
                while !stop_ticker.load(Ordering::SeqCst) {
                    thread::sleep(POLL);
                    ticks += 1;
                    if ticks % TICKS_PER_EXPIRY == 0 {
                        vaults.expire();
                    }
                }
            })?,
    );
    Ok(Running {
        address: local,
        stop,
        threads,
    })
}

/// Boucle d'un fil de travail : reçoit une requête, la route, répond.
fn work(server: &Server, stop: &AtomicBool, context: &Context) {
    while !stop.load(Ordering::SeqCst) {
        match server.recv_timeout(POLL) {
            Ok(Some(incoming)) => answer(incoming, context),
            Ok(None) => {}
            Err(error) => {
                eprintln!("réception impossible : {error}");
                break;
            }
        }
    }
}

/// Traite une requête et écrit la réponse.
fn answer(mut incoming: tiny_http::Request, context: &Context) {
    let sent = match read(&mut incoming) {
        Ok(request) => incoming.respond(convert(route(&request, context))),
        Err(status) => incoming.respond(bare(status)),
    };
    if let Err(error) = sent {
        eprintln!("réponse non écrite : {error}");
    }
}
