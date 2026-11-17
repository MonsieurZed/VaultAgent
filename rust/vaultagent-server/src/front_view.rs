//! Assemblage de la page complète : les quatre blocs de la page principale, le mode « Modifier » ou l'aide, sans
//! aucun script.

use crate::front_account::account;
use crate::front_add::add;
use crate::front_bar::{back_bar, status_bar};
use crate::front_devices::devices;
use crate::front_help::help;
use crate::front_html::View;
use crate::front_pending::{has_pending, pending};
use crate::front_screen::Screen;
use crate::front_vault_block::vault_block;

const REFRESH_SECONDS: u32 = 10;

/// La page se recharge-t-elle seule : seulement la page principale, quand aucune demande n'attend, qu'aucun message
/// n'est affiché et qu'aucun retrait n'attend sa confirmation (un code 2FA en cours de saisie ne doit pas disparaître).
fn refreshes(view: &View) -> bool {
    view.screen == Screen::Main && !has_pending(view) && view.notice.is_none() && view.confirm.is_none()
}

/// Barre et contenu de la page selon l'écran.
fn parts(view: &View) -> (String, String) {
    match view.screen {
        Screen::Main | Screen::Vault => {
            let (class, first) = if view.screen == Screen::Vault {
                ("blocks edit", account(view))
            } else {
                ("blocks", vault_block(view))
            };
            (
                status_bar(view),
                format!(
                    "<div class=\"{class}\">{first}{}{}{}</div>",
                    pending(view),
                    devices(view),
                    add(view)
                ),
            )
        }
        Screen::Help => (back_bar(view), help(view)),
    }
}

/// Rend la page complète.
///
/// Paramètres : le contexte de rendu (langue, écran, message, état du coffre).
/// Retour : le document HTML.
#[must_use]
pub fn render(view: &View) -> String {
    let refresh = if refreshes(view) {
        format!("<meta http-equiv=\"refresh\" content=\"{REFRESH_SECONDS}\">")
    } else {
        String::new()
    };
    let (bar, content) = parts(view);
    format!(
        "<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
content=\"width=device-width,initial-scale=1\">{refresh}<title>VaultAgent</title><link rel=\"stylesheet\" href=\"/style.css\">\
</head>\n<body>{bar}<main>{content}</main></body></html>\n",
        view.lang.code(),
    )
}
