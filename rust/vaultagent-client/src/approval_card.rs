//! Carte d'approbation (mode carte) : petite fenêtre en haut à droite de la zone de travail, au-dessus des autres
//! fenêtres, sans focus ni bouton de barre des tâches, qui se ferme à l'échéance de la demande.

use std::sync::mpsc::Sender;

use vaultagent_win32::{
    Control, Poster, UiEvent, UiHandler, Win32Error, WindowKind, WindowRef, WindowSpec, create_window, work_area,
};

use crate::translate::Translator;
use crate::tray_jobs::{CODE_CARD_CLOSED, Job};
use crate::tray_select::Presentation;

const ID_YES: u16 = 1;
const ID_NO: u16 = 2;
const TIMER_EXPIRY: usize = 1;
const CARD_WIDTH: i32 = 436;
const CARD_HEIGHT: i32 = 250;
const MARGIN: i32 = 20;

/// Textes et boutons que la demande impose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardPlan {
    /// Texte du bouton « Oui » (français, à traduire).
    pub yes_key: &'static str,
    /// Vrai si le bouton « Oui » est plus large (libellé long).
    pub wide_yes: bool,
    /// Vrai si la demande exige Hello sans clé : « Oui » et la case sont désactivés.
    pub blocked: bool,
}

/// Choisit les boutons de la carte.
///
/// Paramètres : la demande présentée.
/// Retour : le libellé de « Oui » (Hello, validation Windows ou simple), sa largeur et le blocage faute de clé Hello.
#[must_use]
pub fn plan_for(presentation: &Presentation) -> CardPlan {
    let request = &presentation.request.request;
    let has_key = request.hello_credential.as_ref().is_some_and(|value| !value.is_null());
    if request.hello_required {
        return CardPlan {
            yes_key: "Oui avec Windows Hello",
            wide_yes: true,
            blocked: !has_key,
        };
    }
    if request.elevation_required {
        return CardPlan {
            yes_key: "Oui avec validation Windows",
            wide_yes: true,
            blocked: false,
        };
    }
    CardPlan {
        yes_key: "Oui",
        wide_yes: false,
        blocked: false,
    }
}

/// Gestionnaire des événements de la carte.
struct CardHandler {
    presentation: Presentation,
    plan: CardPlan,
    tr: Translator,
    jobs: Sender<Job>,
    tray: Poster,
    always: Option<WindowRef>,
    buttons: Vec<WindowRef>,
    title: Option<WindowRef>,
    decided: bool,
}

impl CardHandler {
    /// Crée les contrôles de la carte.
    fn build(&mut self, window: WindowRef) {
        let tr = &self.tr;
        let request = &self.presentation.request;
        let vault = request
            .vault_name
            .clone()
            .map_or_else(String::new, |name| format!("{name} · "));
        let title = window.add_control(
            Control::Label,
            &tr.t("Autoriser l'accès au coffre ?"),
            0,
            (20, 18, 380, 28),
        );
        window
            .add_control(
                Control::Label,
                &format!("{vault}{}", request.device_name),
                0,
                (20, 55, 380, 35),
            )
            .ok();
        self.always = window
            .add_control(
                Control::Checkbox,
                &tr.t("Toujours accepter les demandes de ce client"),
                0,
                (20, 100, 380, 28),
            )
            .ok();
        let (x, width) = if self.plan.wide_yes { (100, 190) } else { (200, 90) };
        let yes = window
            .add_control(Control::Button, &tr.t(self.plan.yes_key), ID_YES, (x, 155, width, 34))
            .ok();
        let no = window
            .add_control(Control::DefaultButton, &tr.t("Non"), ID_NO, (300, 155, 90, 34))
            .ok();
        self.buttons = [yes, no].into_iter().flatten().collect();
        self.title = title.ok();
        if self.plan.blocked {
            yes.into_iter()
                .chain(self.always)
                .for_each(|control| control.set_enabled(false));
            if let Some(label) = self.title {
                label.set_text(&tr.t("Windows Hello requis : activez-le dans le bouclier."));
            }
        }
        window.set_timer(TIMER_EXPIRY, 1000);
    }

    /// Envoie la décision et verrouille la carte en attendant la réponse.
    fn decide(&mut self, approve: bool) {
        if self.decided {
            return;
        }
        self.decided = true;
        let remember = approve && self.always.is_some_and(WindowRef::is_checked);
        let job = Job::Decide {
            request: Box::new(self.presentation.request.clone()),
            mode: self.presentation.mode,
            approve,
            remember,
        };
        self.buttons
            .iter()
            .chain(self.always.iter())
            .for_each(|control| control.set_enabled(false));
        if self.jobs.send(job).is_err() {
            self.decided = false;
        }
    }
}

impl UiHandler for CardHandler {
    /// Crée les contrôles, décide sur clic, ferme à l'échéance ou à la demande.
    fn handle(&mut self, window: WindowRef, event: UiEvent) {
        match event {
            UiEvent::Created => self.build(window),
            UiEvent::Command(ID_YES) => self.decide(true),
            UiEvent::Command(ID_NO) => self.decide(false),
            UiEvent::Timer(TIMER_EXPIRY) => {
                let expired = self.presentation.request.is_expired(crate::tray_clock::now_unix());
                if expired && !self.decided {
                    window.destroy();
                }
            }
            UiEvent::CloseRequested => window.destroy(),
            UiEvent::Destroyed => {
                window.kill_timer(TIMER_EXPIRY);
                self.tray.post(CODE_CARD_CLOSED);
            }
            _ => {}
        }
    }
}

/// Ouvre la carte d'approbation sans prendre le focus.
///
/// Paramètres : la fenêtre de barre à prévenir à la fermeture, le canal des travaux, le traducteur et la demande.
/// Retour : la carte affichée, ou l'erreur `Api` de sa création.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn open_card(
    tray: Poster,
    jobs: Sender<Job>,
    tr: &Translator,
    presentation: Presentation,
) -> Result<WindowRef, Win32Error> {
    let (_, top, right, _) = work_area().unwrap_or((0, 0, 1280, 720));
    let spec = WindowSpec {
        class: "VaultAgentApprovalCard",
        title: "VaultAgent",
        kind: WindowKind::Card,
        rect: (right - CARD_WIDTH - MARGIN, top + MARGIN, CARD_WIDTH, CARD_HEIGHT),
    };
    let handler = CardHandler {
        plan: plan_for(&presentation),
        presentation,
        tr: tr.clone(),
        jobs,
        tray,
        always: None,
        buttons: Vec::new(),
        title: None,
        decided: false,
    };
    let window = create_window(&spec, Box::new(handler))?;
    window.show(false);
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray_select::ValidationMode;
    use crate::tray_status::TrayRequest;

    /// Construit une demande de test.
    fn presentation(hello: bool, elevation: bool, credential: bool) -> Presentation {
        let body = serde_json::json!({
            "requestId": "r", "challenge": "c", "deviceName": "PC",
            "helloRequired": hello, "elevationRequired": elevation,
            "helloCredential": if credential { serde_json::json!({"id": "x"}) } else { serde_json::Value::Null },
        });
        Presentation {
            request: serde_json::from_value::<TrayRequest>(body).unwrap(),
            mode: ValidationMode::Card,
        }
    }

    #[test]
    fn plain_request_has_a_plain_yes() {
        let plan = plan_for(&presentation(false, false, false));
        assert_eq!((plan.yes_key, plan.wide_yes, plan.blocked), ("Oui", false, false));
    }

    #[test]
    fn hello_request_says_so_and_is_blocked_without_key() {
        let ready = plan_for(&presentation(true, false, true));
        assert_eq!((ready.yes_key, ready.blocked), ("Oui avec Windows Hello", false));
        assert!(plan_for(&presentation(true, false, false)).blocked);
    }

    #[test]
    fn elevation_request_names_the_windows_validation() {
        let plan = plan_for(&presentation(false, true, false));
        assert_eq!((plan.yes_key, plan.wide_yes), ("Oui avec validation Windows", true));
    }

    #[test]
    #[ignore = "ouvre une vraie carte a l'ecran"]
    fn real_card_opens_and_closes() {
        let (jobs, _receiver) = std::sync::mpsc::channel();
        let tr = Translator::new(crate::translate::Language::French);
        let host = vaultagent_win32::create_window(
            &WindowSpec {
                class: "VaultAgentCardTestHost",
                title: "h",
                kind: WindowKind::Hidden,
                rect: (0, 0, 10, 10),
            },
            Box::new(Quiet),
        )
        .unwrap();
        let card = open_card(host.poster(), jobs, &tr, presentation(false, false, false)).unwrap();
        card.destroy();
        host.destroy();
    }

    struct Quiet;

    impl UiHandler for Quiet {
        /// Ignore tout.
        fn handle(&mut self, _window: WindowRef, _event: UiEvent) {}
    }
}
