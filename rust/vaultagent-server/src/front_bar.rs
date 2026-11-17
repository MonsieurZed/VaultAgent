//! Barre du haut : nom, pastille d'état, déverrouillage ou verrouillage, langue et aide ; barre réduite des pages
//! secondaires avec le lien « Retour ».

use crate::front_dates::time_of;
use crate::front_escape::escape;
use crate::front_html::{View, disabled, form_open};
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

/// Pastille d'état : « À configurer », « Verrouillé » ou « Déverrouillé jusqu'à HH:MM » (heure de Paris).
fn pill(view: &View) -> String {
    let (session, lang) = (&view.state.session, view.lang);
    let label = if !session.configured {
        text(lang, "À configurer")
    } else if !session.unlocked {
        text(lang, "Verrouillé")
    } else if let Some(iso) = &session.expires_utc {
        format!("{} {}", text(lang, "Déverrouillé jusqu’à"), escape(&time_of(iso)))
    } else {
        text(lang, "Déverrouillé")
    };
    let dot = if session.unlocked { "●" } else { "○" };
    format!("<span class=\"pill\"><span class=\"dot\">{dot}</span> <strong>{label}</strong></span>")
}

/// Formulaire « Déverrouiller » avec son champ 2FA, ou « Verrouiller » quand le coffre est ouvert.
fn control(view: &View) -> String {
    let (session, lang) = (&view.state.session, view.lang);
    if session.unlocked {
        return format!(
            "{}<button type=\"submit\" class=\"quiet\">{}</button></form>",
            form_open("lock", "inline"),
            text(lang, "Verrouiller")
        );
    }
    format!(
        "{}<input name=\"code\" type=\"text\" autocomplete=\"one-time-code\" placeholder=\"{}\" aria-label=\"2FA\" \
value=\"\"><button type=\"submit\"{}>{}</button></form>",
        form_open("unlock", "inline"),
        text(lang, "Code 2FA · facultatif"),
        disabled(!session.configured),
        text(lang, "Déverrouiller"),
    )
}

/// Barre d'état de la page principale.
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la barre.
#[must_use]
pub fn status_bar(view: &View) -> String {
    format!(
        "<header class=\"bar\">{BRAND}{}<div class=\"actions\">{}</div><div class=\"header-links\">{}\
<a class=\"refresh\" href=\"{}\">{}</a><a href=\"{}\">{}</a></div></header>\n",
        pill(view),
        control(view),
        languages(view),
        view.screen.href_html(&[]),
        text(view.lang, "Actualiser"),
        Screen::Help.href_html(&[]),
        text(view.lang, "Aide"),
    )
}

/// Barre d'une page secondaire : nom, langue et lien « Retour ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la barre.
#[must_use]
pub fn back_bar(view: &View) -> String {
    format!(
        "<header class=\"bar\">{BRAND}<div class=\"header-links\">{}<a class=\"back\" href=\"/\">{}</a></div></header>\n",
        languages(view),
        text(view.lang, "Retour"),
    )
}
