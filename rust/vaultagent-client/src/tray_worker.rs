//! Fil réseau de l'application de barre : exécute les travaux hors de la boucle de messages (réseau, invites UAC et
//! Hello) puis dépose les réponses dans la boîte et réveille la fenêtre.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

use vaultagent_win32::Poster;

use crate::approval_request::ApprovalRequest;
use crate::config::ClientConfig;
use crate::config_store::ConfigStore;
use crate::decide::Approver;
use crate::elevation_system::{ElevationSystem, system_elevation};
use crate::error::ClientError;
use crate::hello_system::{HelloSystem, system_hello};
use crate::https_transport::HttpsTransport;
use crate::pairing_flow::pair_and_save;
use crate::pause::ThreadPause;
use crate::proofs::SystemProofs;
use crate::server_address::ServerAddress;
use crate::translate::{Language, Translator};
use crate::tray_actions::Decision;
use crate::tray_jobs::{Job, Mailbox, NoticeSink, Reply, deliver};
use crate::tray_select::ValidationMode;
use crate::tray_status::{TrayRequest, fetch_snapshots};

/// Ce que le fil réseau doit connaître du poste.
#[derive(Debug, Clone)]
pub struct WorkerSetup {
    /// Fichier de configuration explicite (`--config`).
    pub config: Option<PathBuf>,
    /// Dossier personnel.
    pub home: Option<OsString>,
    /// Dossier de données de l'application.
    pub local_data: Option<PathBuf>,
    /// Dossier temporaire.
    pub temp_dir: PathBuf,
    /// Nom de la machine.
    pub machine_name: String,
    /// Langue de l'interface.
    pub language: Language,
}

/// Environnement du fil : stockage, systèmes d'élévation et de Hello, traducteur.
struct Env {
    setup: WorkerSetup,
    store: ConfigStore,
    elevation: Box<dyn ElevationSystem>,
    hello: Box<dyn HelloSystem>,
    tr: Translator,
}

impl Env {
    /// Construit l'environnement.
    fn new(setup: &WorkerSetup) -> Result<Self, ClientError> {
        Ok(Self {
            store: ConfigStore::locate(
                setup.config.as_deref(),
                setup.home.as_deref(),
                setup.local_data.as_deref(),
            )?,
            elevation: system_elevation(),
            hello: system_hello(),
            tr: Translator::new(setup.language),
            setup: setup.clone(),
        })
    }

    /// Exécute une action avec le transport, l'approbateur et les preuves du système.
    fn with_approver<T>(
        &self,
        action: impl FnOnce(&Approver<'_>, &SystemProofs<'_>) -> Result<T, ClientError>,
    ) -> Result<T, ClientError> {
        let config: ClientConfig = self.store.load()?;
        let key = self.store.approver_key()?;
        let transport = HttpsTransport::new(config.server.clone());
        let approver = Approver {
            transport: &transport,
            config: &config,
            key: &key,
        };
        let proofs = SystemProofs::new(self.elevation.as_ref(), self.hello.as_ref(), &self.setup.temp_dir);
        action(&approver, &proofs)
    }

    /// Traduit un texte.
    fn t(&self, key: &str) -> String {
        self.tr.t(key)
    }

    /// Bulle d'information.
    fn notice(&self, key: &str, warning: bool) -> Reply {
        Reply::Notice {
            text: self.t(key),
            warning,
        }
    }
}

/// Lit l'état de tous les coffres.
fn refresh(env: &Env) -> Reply {
    let Ok(config) = env.store.load() else {
        return Reply::NotPaired;
    };
    let transport = HttpsTransport::new(config.server.clone());
    match fetch_snapshots(&transport, &config.token) {
        Ok(vaults) => Reply::Snapshots {
            server: config.server.as_str().to_owned(),
            vaults,
        },
        Err(_) => Reply::Unavailable,
    }
}

/// Décide une demande selon le mode de son coffre.
fn decide(env: &Env, request: &TrayRequest, mode: ValidationMode, approve: bool, remember: bool) -> Reply {
    let result =
        env.with_approver(|approver, proofs| decide_with(approver, proofs, &request.request, mode, approve, remember));
    Reply::Decided {
        key: request.key(),
        result,
    }
}

/// Décision en une étape (mode UAC) ou selon le choix de la carte.
fn decide_with(
    approver: &Approver<'_>,
    proofs: &SystemProofs<'_>,
    request: &ApprovalRequest,
    mode: ValidationMode,
    approve: bool,
    remember: bool,
) -> Result<Decision, ClientError> {
    if mode == ValidationMode::Uac {
        return approver.decide_one_step(request, proofs);
    }
    approver.decide_unlock(request, approve, remember, proofs)?;
    Ok(if approve { Decision::Approved } else { Decision::Denied })
}

/// Texte de la bulle qui suit un échec de Hello.
fn hello_failure(error: &ClientError) -> &'static str {
    match error {
        ClientError::HelloUnavailable => {
            "Windows Hello indisponible. Configurez-le dans Windows depuis une session locale ; le RDP peut empêcher son utilisation."
        }
        _ => "Windows Hello annulé ou opération refusée. Aucune validation de secours automatique.",
    }
}

/// Bascule la validation et rend la bulle d'information.
fn switch(env: &Env, target: ValidationMode) -> Reply {
    match env.with_approver(|approver, proofs| approver.switch_validation(target, proofs)) {
        Ok(done) if done.changed == 0 && target == ValidationMode::Uac => {
            env.notice("Rien à activer : PC non approbateur ou déjà activé", false)
        }
        Ok(done) if done.changed == 0 => env.notice("Validation inchangée", false),
        Ok(_) if target == ValidationMode::Uac => env.notice("Validation par UAC activée", false),
        Ok(_) => env.notice("Validation par carte activée", false),
        Err(_) => env.notice("Validation inchangée", true),
    }
}

/// Association de ce client, sur un fil à part car elle attend l'approbation du serveur.
fn pair(setup: &WorkerSetup, mailbox: &Mailbox, poster: Poster, address: &str, approver_key: bool) {
    let tr = Translator::new(setup.language);
    let outcome = Env::new(setup).and_then(|env| {
        let server = ServerAddress::parse(address)?;
        let transport = HttpsTransport::new(server.clone());
        let mut sink = NoticeSink::new(mailbox.clone(), poster);
        pair_and_save(
            &env.store,
            &transport,
            server,
            &setup.machine_name,
            approver_key,
            &mut ThreadPause,
            &mut sink,
        )
    });
    let reply = match outcome {
        Ok(_) => Reply::Paired {
            text: tr.t("Client associé. Le jeton est protégé par Windows pour votre compte. Autorisez les clients depuis le serveur pour remplir les champs."),
            ok: true,
        },
        Err(_) => Reply::Paired {
            text: tr.t("Demande refusée ou expirée. Vérifiez l’adresse HTTPS du serveur."),
            ok: false,
        },
    };
    deliver(mailbox, poster, reply);
}

/// Exécute un travail et rend ses réponses.
fn execute(env: &Env, job: Job) -> Vec<Reply> {
    match job {
        Job::Refresh => vec![refresh(env)],
        Job::Decide {
            request,
            mode,
            approve,
            remember,
        } => vec![decide(env, &request, mode, approve, remember), refresh(env)],
        Job::Lock => {
            let notice = env.with_approver(|approver, _| approver.lock_all());
            let mut replies = notice
                .err()
                .map(|_| env.notice("Service du coffre indisponible", true))
                .into_iter()
                .collect::<Vec<_>>();
            replies.push(refresh(env));
            replies
        }
        Job::ResetAutomatic => {
            let reply = match env.with_approver(|approver, proofs| approver.reset_all_automatic(proofs)) {
                Ok(_) => env.notice("Acceptation automatique désactivée", false),
                Err(_) => env.notice("Service du coffre indisponible", true),
            };
            vec![reply, refresh(env)]
        }
        Job::Switch(target) => vec![switch(env, target), refresh(env)],
        Job::EnrollHello => {
            let reply = match env.with_approver(|approver, _| approver.enroll_hello(env.hello.as_ref(), None)) {
                Ok(_) => env.notice("Windows Hello activé sur les coffres autorisés.", false),
                Err(error) => env.notice(hello_failure(&error), true),
            };
            vec![reply, refresh(env)]
        }
        Job::Pair { .. } | Job::Stop => Vec::new(),
    }
}

/// Démarre le fil réseau.
///
/// Paramètres : le poste, le récepteur des travaux, la boîte des réponses et la fenêtre à réveiller.
/// Retour : la poignée du fil ; il s'arrête sur `Job::Stop` ou quand l'émetteur disparaît.
#[must_use]
pub fn spawn(setup: WorkerSetup, jobs: Receiver<Job>, mailbox: Mailbox, poster: Poster) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let Ok(env) = Env::new(&setup) else {
            deliver(&mailbox, poster, Reply::Unavailable);
            return;
        };
        loop {
            let next = jobs.recv();
            let Ok(job) = next else { return };
            match job {
                Job::Stop => return,
                Job::Pair { server, approver } => {
                    let (setup, mailbox) = (setup.clone(), mailbox.clone());
                    std::thread::spawn(move || pair(&setup, &mailbox, poster, &server, approver));
                }
                other => {
                    for reply in execute(&env, other) {
                        deliver(&mailbox, poster, reply);
                    }
                }
            }
        }
    })
}
