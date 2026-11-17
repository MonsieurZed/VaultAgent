//! Barre fine du haut : marque, message de la dernière action, langues, « Actualiser » et « Aide » ; barre réduite
//! de l'aide avec le lien « Retour ».

use crate::front_html::View;
use crate::front_lang::Lang;
use crate::front_screen::Screen;
use crate::i18n::text;

const BRAND: &str = "<a class=\"brand\" href=\"/\"><span class=\"shield\">◆</span> VaultAgent</a>";

/// Liste de liens de langue : chacun garde la page courante.
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
                "<a href=\"{}\" hreflang=\"{}\"{current}>{}</a>",
                view.screen.href_html(&[("lang", lang.code())]),
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

/// Message de la dernière action, ou le texte vide.
fn message(view: &View) -> String {
    view.notice.map_or_else(String::new, |notice| {
        format!(
            "<p class=\"message\" role=\"status\">{}</p>",
            text(view.lang, notice.label())
        )
    })
}

/// Barre de la page principale et du mode « Modifier ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la barre.
#[must_use]
pub fn status_bar(view: &View) -> String {
    format!(
        "<header class=\"bar\">{BRAND}{}<div class=\"header-links\">{}\
<a class=\"refresh\" href=\"{}\">{}</a><a href=\"{}\">{}</a></div></header>\n",
        message(view),
        languages(view),
        view.screen.href_html(&[]),
        text(view.lang, "Actualiser"),
        Screen::Help.href_html(&[]),
        text(view.lang, "Aide"),
    )
}

/// Barre de l'aide : marque, message, langues et lien « Retour ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la barre.
#[must_use]
pub fn back_bar(view: &View) -> String {
    format!(
        "<header class=\"bar\">{BRAND}{}<div class=\"header-links\">{}<a class=\"back\" href=\"/\">{}</a></div></header>\n",
        message(view),
        languages(view),
        text(view.lang, "Retour"),
    )
}
