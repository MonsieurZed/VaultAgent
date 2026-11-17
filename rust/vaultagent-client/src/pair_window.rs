//! Fenêtre d'association : adresse HTTPS, case « approbateur », bouton de demande et affichage du code de contrôle.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use vaultagent_win32::{
    Control, Poster, UiEvent, UiHandler, Win32Error, WindowKind, WindowRef, WindowSpec, create_window, work_area,
};

use crate::translate::Translator;
use crate::tray_jobs::{CODE_PAIR_CLOSED, Job};

const ID_REQUEST: u16 = 1;
const WIDTH: i32 = 500;
const HEIGHT: i32 = 370;

/// Contrôles de la fenêtre que la fenêtre de barre met à jour à la réception des réponses.
#[derive(Default)]
pub struct PairView {
    status: Cell<Option<WindowRef>>,
    button: Cell<Option<WindowRef>>,
}

impl PairView {
    /// Affiche un texte dans la zone d'état.
    ///
    /// Paramètres : le texte.
    pub fn set_status(&self, text: &str) {
        if let Some(label) = self.status.get() {
            label.set_text(text);
        }
    }

    /// Active ou désactive le bouton de demande.
    ///
    /// Paramètres : vrai pour activer.
    pub fn set_button_enabled(&self, enabled: bool) {
        if let Some(button) = self.button.get() {
            button.set_enabled(enabled);
        }
    }
}

/// Gestionnaire des événements de la fenêtre.
struct PairHandler {
    tr: Translator,
    jobs: Sender<Job>,
    tray: Poster,
    view: Rc<PairView>,
    initial_server: String,
    address: Option<WindowRef>,
    approver: Option<WindowRef>,
}

impl PairHandler {
    /// Crée les contrôles de la fenêtre.
    fn build(&mut self, window: WindowRef) {
        let tr = &self.tr;
        window
            .add_control(
                Control::Label,
                &tr.t("Connecter ce PC au serveur"),
                0,
                (20, 15, 450, 28),
            )
            .ok();
        window
            .add_control(
                Control::Label,
                &tr.t("Aucun service Windows requis. Donnez l’adresse du serveur et demandez l’association : vous l’autorisez ensuite dans l’onglet Clients du serveur. Ne saisissez pas le mot de passe du coffre ici."),
                0,
                (20, 50, 450, 90),
            )
            .ok();
        window
            .add_control(
                Control::Label,
                &tr.t("Adresse HTTPS du serveur VaultAgent"),
                0,
                (20, 150, 450, 22),
            )
            .ok();
        self.address = window
            .add_control(Control::Edit, &self.initial_server, 0, (20, 176, 450, 26))
            .ok();
        self.approver = window
            .add_control(
                Control::Checkbox,
                &tr.t("Ce PC peut approuver les déverrouillages (Hello, UAC)"),
                0,
                (20, 212, 450, 24),
            )
            .ok();
        if let Some(check) = self.approver {
            check.set_checked(true);
        }
        let button = window
            .add_control(
                Control::DefaultButton,
                &tr.t("Demander l’association"),
                ID_REQUEST,
                (20, 248, 220, 34),
            )
            .ok();
        self.view.button.set(button);
        self.view
            .status
            .set(window.add_control(Control::Label, "", 0, (20, 290, 450, 64)).ok());
    }

    /// Envoie la demande d'association au fil réseau.
    fn request(&self) {
        let server = self.address.map_or_else(String::new, WindowRef::text);
        let approver = self.approver.is_some_and(WindowRef::is_checked);
        self.view.set_button_enabled(false);
        if self.jobs.send(Job::Pair { server, approver }).is_err() {
            self.view.set_button_enabled(true);
        }
    }
}

impl UiHandler for PairHandler {
    /// Crée les contrôles, envoie la demande sur clic, prévient la barre à la fermeture.
    fn handle(&mut self, window: WindowRef, event: UiEvent) {
        match event {
            UiEvent::Created => self.build(window),
            UiEvent::Command(ID_REQUEST) => self.request(),
            UiEvent::CloseRequested => window.destroy(),
            UiEvent::Destroyed => {
                self.view.status.set(None);
                self.view.button.set(None);
                self.tray.post(CODE_PAIR_CLOSED);
            }
            _ => {}
        }
    }
}

/// Ouvre la fenêtre d'association au centre de la zone de travail.
///
/// Paramètres : la fenêtre de barre à prévenir, le canal des travaux, le traducteur, la vue partagée et l'adresse
/// de départ.
/// Retour : la fenêtre affichée, ou l'erreur `Api` de sa création.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn open_pair_window(
    tray: Poster,
    jobs: Sender<Job>,
    tr: &Translator,
    view: Rc<PairView>,
    initial_server: &str,
) -> Result<WindowRef, Win32Error> {
    let (left, top, right, bottom) = work_area().unwrap_or((0, 0, 1280, 720));
    let spec = WindowSpec {
        class: "VaultAgentPairWindow",
        title: &tr.t("Association du client"),
        kind: WindowKind::Dialog,
        rect: (
            left + (right - left - WIDTH) / 2,
            top + (bottom - top - HEIGHT) / 2,
            WIDTH,
            HEIGHT,
        ),
    };
    let handler = PairHandler {
        tr: tr.clone(),
        jobs,
        tray,
        view,
        initial_server: initial_server.to_owned(),
        address: None,
        approver: None,
    };
    let window = create_window(&spec, Box::new(handler))?;
    window.show(true);
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_without_window_ignores_updates() {
        let view = PairView::default();
        view.set_status("texte");
        view.set_button_enabled(true);
        assert!(view.status.get().is_none() && view.button.get().is_none());
    }

    #[test]
    #[ignore = "ouvre une vraie fenetre d'association"]
    fn real_window_opens_and_closes() {
        let (jobs, _receiver) = std::sync::mpsc::channel();
        let tr = Translator::new(crate::translate::Language::French);
        let host = vaultagent_win32::create_window(
            &WindowSpec {
                class: "VaultAgentPairTestHost",
                title: "h",
                kind: WindowKind::Hidden,
                rect: (0, 0, 10, 10),
            },
            Box::new(Quiet),
        )
        .unwrap();
        let view = Rc::new(PairView::default());
        let window = open_pair_window(host.poster(), jobs, &tr, view.clone(), "https://vault.example.test").unwrap();
        assert!(view.status.get().is_some());
        window.destroy();
        host.destroy();
    }

    struct Quiet;

    impl UiHandler for Quiet {
        /// Ignore tout.
        fn handle(&mut self, _window: WindowRef, _event: UiEvent) {}
    }
}
