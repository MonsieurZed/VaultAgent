//! Assemblage de la page complète : en-tête, statut, message, demandes, onglets et pied de page, sans aucun script.

use crate::front_about::about;
use crate::front_account::account;
use crate::front_clients::clients;
use crate::front_html::View;
use crate::front_lang::Lang;
use crate::front_status::{status, unlock_requests};
use crate::front_tab::Tab;
use crate::i18n::text;

const REFRESH_SECONDS: u32 = 5;

/// La page se recharge-t-elle seule : jamais sur « Compte » (une saisie serait perdue) ; sur « Clients » tant
/// qu'aucun message d'action n'est affiché ; et sur tout autre onglet quand des demandes attendent une décision.
fn refreshes(view: &View) -> bool {
    let pending = !view.state.unlock_requests.is_empty() || !view.state.pair_requests.is_empty();
    match view.tab {
        Tab::Account => false,
        Tab::Clients => view.notice.is_none() || pending,
        Tab::About => pending,
    }
}

/// Liste de liens de langue : chacun garde l'onglet courant.
fn languages(view: &View) -> String {
    let links = Lang::ALL
        .iter()
        .map(|lang| {
            let current = if *lang == view.lang {
                " aria-current=\"true\" class=\"selected\""
            } else {
                ""
            };
            format!(
                "<a href=\"/?tab={}&amp;lang={}\" hreflang=\"{}\"{current}>{}</a>",
                view.tab.code(),
                lang.code(),
                lang.code(),
                lang.label()
            )
        })
        .collect::<Vec<String>>()
        .concat();
    format!(
        "<nav class=\"languages\" aria-label=\"{}\">{links}</nav>",
        text(view.lang, "Langue")
    )
}

/// Liens des onglets ; l'onglet actif porte la classe `selected`.
fn tabs(view: &View) -> String {
    let links = Tab::ALL
        .iter()
        .map(|tab| {
            let current = if *tab == view.tab {
                " class=\"selected\" aria-current=\"page\""
            } else {
                ""
            };
            format!(
                "<a href=\"/?tab={}\"{current}>{}</a>",
                tab.code(),
                text(view.lang, tab.label())
            )
        })
        .collect::<Vec<String>>()
        .concat();
    format!("<nav class=\"tabs\" aria-label=\"Settings\">{links}</nav>")
}

/// Message de la dernière action, ou le texte vide.
fn message(view: &View) -> String {
    view.notice.map_or_else(
        || "<p class=\"message\" role=\"status\"></p>".to_owned(),
        |notice| {
            format!(
                "<p class=\"message\" role=\"status\">{}</p>",
                text(view.lang, notice.label())
            )
        },
    )
}

/// Rend la page complète.
///
/// Paramètres : le contexte de rendu (langue, onglet, message, état du coffre).
/// Retour : le document HTML.
#[must_use]
pub fn render(view: &View) -> String {
    let lang = view.lang;
    let refresh = if refreshes(view) {
        format!("<meta http-equiv=\"refresh\" content=\"{REFRESH_SECONDS}\">")
    } else {
        String::new()
    };
    let body = match view.tab {
        Tab::Account => account(view),
        Tab::Clients => clients(view),
        Tab::About => about(view),
    };
    format!(
        "<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
content=\"width=device-width,initial-scale=1\">{refresh}<title>VaultAgent</title><link rel=\"stylesheet\" href=\"/style.css\">\
</head>\n<body><header><a class=\"brand\" href=\"/?tab={}\"><span class=\"shield\">◆</span> VaultAgent <small>Linux · preview</small></a>\
<div class=\"header-links\"><a class=\"refresh\" href=\"/?tab={}\">{}</a>{}</div></header>\n<main><section class=\"intro\">\
<p class=\"eyebrow\">{}</p><h1>{}</h1><p>{}</p></section>\n{}{}{}{}{}</main>\n<footer>VaultAgent · {} · {}</footer></body></html>\n",
        lang.code(),
        view.tab.code(),
        view.tab.code(),
        text(lang, "Actualiser"),
        languages(view),
        text(lang, "Votre coffre, disponible sans votre PC"),
        text(lang, "Un coffre. Vos clients. Le contrôle reste à vous."),
        text(
            lang,
            "Le serveur conserve le moteur du coffre. Les clients remplissent les champs sans afficher les secrets aux agents."
        ),
        status(view),
        message(view),
        unlock_requests(view),
        tabs(view),
        body,
        crate::front_escape::escape(view.state.session.version),
        text(lang, "Accès privé LAN / VPN"),
    )
}
