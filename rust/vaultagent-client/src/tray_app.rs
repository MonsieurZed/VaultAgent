//! Fenêtre cachée de l'application de barre : icône, menu, interrogation toutes les 5 secondes, présentation des
//! demandes (bulle puis invite UAC, ou carte) et traitement des réponses du fil réseau.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use vaultagent_win32::{
    BalloonKind, Icon, MenuEntry, TrayAction, TrayIcon, UiEvent, UiHandler, WindowRef, open_url, quit, show_menu,
};

use crate::approval_card::open_card;
use crate::pair_window::{PairView, open_pair_window};
use crate::translate::Translator;
use crate::tray_clock::now_unix;
use crate::tray_jobs::{CODE_CARD_CLOSED, CODE_PAIR_CLOSED, CODE_REPLY, Job, Mailbox, Reply, drain};
use crate::tray_menu::{MenuId, MenuLine, MenuState, menu_lines};
use crate::tray_select::{
    Overview, Presentation, ValidationMode, latest_open, overall_mode, overview_of, pick_new, status_text, tip_of,
};
use crate::tray_status::VaultSnapshot;

const TIMER_REFRESH: usize = 1;
const REFRESH_MILLIS: u32 = 5000;

/// État de l'application de barre.
pub struct TrayApp {
    tr: Translator,
    icon: Icon,
    tray: Option<TrayIcon>,
    jobs: Sender<Job>,
    mailbox: Mailbox,
    snapshots: Vec<VaultSnapshot>,
    overview: Overview,
    server: Option<String>,
    notified: HashSet<String>,
    card: Option<WindowRef>,
    deciding: bool,
    refreshing: bool,
    pair: Option<WindowRef>,
    pair_view: Rc<PairView>,
}

impl TrayApp {
    /// Crée l'état initial.
    ///
    /// Paramètres : le traducteur, l'icône, le canal des travaux et la boîte des réponses.
    /// Retour : l'application, sans fenêtre ni icône de barre avant l'événement `Created`.
    #[must_use]
    pub fn new(tr: Translator, icon: Icon, jobs: Sender<Job>, mailbox: Mailbox) -> Self {
        Self {
            tr,
            icon,
            tray: None,
            jobs,
            mailbox,
            snapshots: Vec::new(),
            overview: Overview::NotPaired,
            server: None,
            notified: HashSet::new(),
            card: None,
            deciding: false,
            refreshing: false,
            pair: None,
            pair_view: Rc::new(PairView::default()),
        }
    }

    /// Demande une relecture si aucune n'est en cours.
    fn refresh(&mut self) {
        if !self.refreshing && self.jobs.send(Job::Refresh).is_ok() {
            self.refreshing = true;
        }
    }

    /// Affiche une bulle d'information.
    fn balloon(&self, text: &str, warning: bool) {
        let kind = if warning {
            BalloonKind::Warning
        } else {
            BalloonKind::Info
        };
        if let Some(tray) = &self.tray {
            tray.balloon("VaultAgent", text, kind).ok();
        }
    }

    /// Met à jour le texte d'état et l'info-bulle.
    fn show_status(&self) {
        if let Some(tray) = &self.tray {
            tray.set_tip(&tip_of(&status_text(&self.tr, self.overview))).ok();
        }
    }

    /// Présente une demande : bulle puis invite UAC directe, ou carte.
    fn present(&mut self, window: WindowRef, presentation: Presentation) {
        if self.card.is_some() || self.deciding {
            return;
        }
        match presentation.mode {
            ValidationMode::Uac => {
                let request = &presentation.request;
                let vault = request
                    .vault_name
                    .clone()
                    .unwrap_or_else(|| request.request.vault().to_owned());
                let text = self.tr.fill(
                    "{client} demande l'accès à {vault}",
                    &[("client", &request.device_name), ("vault", &vault)],
                );
                self.balloon(&text, false);
                let job = Job::Decide {
                    request: Box::new(presentation.request),
                    mode: ValidationMode::Uac,
                    approve: true,
                    remember: false,
                };
                self.deciding = self.jobs.send(job).is_ok();
            }
            ValidationMode::Card => {
                self.card = open_card(window.poster(), self.jobs.clone(), &self.tr, presentation).ok();
            }
        }
    }

    /// Traite une réponse du fil réseau.
    fn on_reply(&mut self, window: WindowRef, reply: Reply) {
        match reply {
            Reply::Snapshots { server, vaults } => {
                self.refreshing = false;
                self.overview = overview_of(&vaults);
                self.server = Some(server);
                self.snapshots = vaults;
                self.show_status();
                let busy = self.card.is_some() || self.deciding;
                if let Some(next) = pick_new(&self.snapshots, &mut self.notified, now_unix(), busy) {
                    self.present(window, next);
                }
            }
            Reply::NotPaired => self.set_unreachable(Overview::NotPaired),
            Reply::Unavailable => self.set_unreachable(Overview::Unavailable),
            Reply::Decided { result, .. } => {
                self.deciding = false;
                if let Some(card) = self.card.take() {
                    card.destroy();
                }
                if result.is_err() {
                    self.balloon(&self.tr.t("Demande expirée ou opération refusée"), true);
                }
            }
            Reply::Notice { text, warning } => self.balloon(&text, warning),
            Reply::Verification(code) => {
                let first = self.tr.t("Code de contrôle : ");
                let last = self
                    .tr
                    .t(". Comparez-le avec celui de l’onglet Clients du serveur, puis cliquez sur Autoriser.");
                self.pair_view.set_status(&format!("{first}{code}{last}"));
            }
            Reply::Paired { text, ok } => {
                self.pair_view.set_status(&text);
                self.pair_view.set_button_enabled(true);
                if ok {
                    self.refresh();
                }
            }
        }
    }

    /// Enregistre un état sans coffre lisible.
    fn set_unreachable(&mut self, overview: Overview) {
        self.refreshing = false;
        self.overview = overview;
        self.snapshots.clear();
        if overview == Overview::NotPaired {
            self.server = None;
        }
        self.show_status();
    }

    /// Affiche le menu et exécute l'entrée choisie.
    fn on_menu(&mut self, window: WindowRef) {
        let state = MenuState {
            status: status_text(&self.tr, self.overview),
            has_request: latest_open(&self.snapshots, now_unix()).is_some(),
            paired: self.server.is_some(),
            mode: overall_mode(&self.snapshots),
        };
        let entries: Vec<MenuEntry> = menu_lines(&self.tr, &state)
            .into_iter()
            .map(|line| match line {
                MenuLine::Item { id, text, enabled } => MenuEntry::Item {
                    id: id as u16,
                    text,
                    enabled,
                },
                MenuLine::Separator => MenuEntry::Separator,
            })
            .collect();
        if let Ok(Some(chosen)) = show_menu(window, &entries)
            && let Some(id) = MenuId::from_u16(chosen)
        {
            self.on_choice(window, id);
        }
    }

    /// Exécute une entrée de menu.
    fn on_choice(&mut self, window: WindowRef, id: MenuId) {
        let send = |job: Job| self.jobs.send(job).is_ok();
        match id {
            MenuId::Request => {
                if let Some(found) = latest_open(&self.snapshots, now_unix()) {
                    self.present(window, found);
                }
            }
            MenuId::OpenServer => {
                if let Some(address) = &self.server {
                    open_url(address).ok();
                }
            }
            MenuId::Pair => self.open_pair(window),
            MenuId::Validation => {
                let target = match overall_mode(&self.snapshots) {
                    Some(ValidationMode::Uac) => ValidationMode::Card,
                    _ => ValidationMode::Uac,
                };
                send(Job::Switch(target));
            }
            MenuId::Hello => {
                send(Job::EnrollHello);
            }
            MenuId::Lock => {
                send(Job::Lock);
            }
            MenuId::ResetAutomatic => {
                send(Job::ResetAutomatic);
            }
            MenuId::Quit => {
                send(Job::Stop);
                window.destroy();
            }
            MenuId::State => {}
        }
    }

    /// Ouvre la fenêtre d'association.
    fn open_pair(&mut self, window: WindowRef) {
        if self.pair.is_some() {
            return;
        }
        let server = self.server.clone().unwrap_or_default();
        self.pair = open_pair_window(
            window.poster(),
            self.jobs.clone(),
            &self.tr,
            self.pair_view.clone(),
            &server,
        )
        .ok();
    }
}

impl UiHandler for TrayApp {
    /// Branche l'icône et la minuterie, route les réponses et les gestes.
    fn handle(&mut self, window: WindowRef, event: UiEvent) {
        match event {
            UiEvent::Created => {
                let tip = tip_of(&status_text(&self.tr, self.overview));
                self.tray = TrayIcon::add(window, &self.icon, &tip).ok();
                window.set_timer(TIMER_REFRESH, REFRESH_MILLIS);
                self.refresh();
            }
            UiEvent::Timer(TIMER_REFRESH) => self.refresh(),
            UiEvent::Posted(CODE_REPLY) => {
                for reply in drain(&self.mailbox) {
                    self.on_reply(window, reply);
                }
            }
            UiEvent::Posted(CODE_CARD_CLOSED) => self.card = None,
            UiEvent::Posted(CODE_PAIR_CLOSED) => {
                self.pair = None;
                self.refresh();
            }
            UiEvent::Tray(TrayAction::Menu) => self.on_menu(window),
            UiEvent::Tray(TrayAction::Open) => self.open_pair(window),
            UiEvent::CloseRequested => window.destroy(),
            UiEvent::Destroyed => {
                window.kill_timer(TIMER_REFRESH);
                self.tray = None;
                quit();
            }
            _ => {}
        }
    }
}
