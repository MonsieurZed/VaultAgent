//! Bandeau de statut du coffre et panneau des demandes de déverrouillage, visibles sur tous les onglets.

use crate::front_dates::time_of;
use crate::front_escape::escape;
use crate::front_html::{View, disabled, form_open, hidden};
use crate::i18n::text;

/// Section de statut : état, expiration, formulaires « Autoriser les clients » et « Verrouiller ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la section.
#[must_use]
pub fn status(view: &View) -> String {
    let (state, lang) = (view.state, view.lang);
    let label = if !state.session.configured {
        "À configurer"
    } else if state.session.unlocked {
        "Coffre disponible pour les clients"
    } else {
        "Coffre verrouillé"
    };
    let dot = if state.session.unlocked { "● " } else { "○ " };
    let expiration = match (state.session.unlocked, &state.session.expires_utc) {
        (false, _) => String::new(),
        (true, Some(iso)) => escape(&time_of(iso)),
        (true, None) => text(lang, "Délai désactivé"),
    };
    let tab = hidden("tab", view.tab.code());
    format!(
        "<section class=\"status\"><div><span class=\"dot\">{dot}</span><strong>{}</strong><p>{expiration}</p></div>\
<div class=\"actions\">{}{tab}<input name=\"code\" type=\"text\" autocomplete=\"one-time-code\" placeholder=\"{}\" \
aria-label=\"2FA\" value=\"\"><button type=\"submit\"{}>{}</button></form>{}{tab}<button type=\"submit\" class=\"quiet\"{}>{}</button></form></div></section>",
        text(lang, label),
        form_open("unlock", "inline"),
        text(lang, "Code 2FA · facultatif"),
        disabled(!state.session.configured),
        text(lang, "Autoriser les clients"),
        form_open("lock", "inline"),
        disabled(!state.session.unlocked),
        text(lang, "Verrouiller"),
    )
}

/// Panneau des demandes de déverrouillage ; vide s'il n'y en a aucune.
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML du panneau, ou le texte vide.
#[must_use]
pub fn unlock_requests(view: &View) -> String {
    if view.state.unlock_requests.is_empty() {
        return String::new();
    }
    let lang = view.lang;
    let rows = view
        .state
        .unlock_requests
        .iter()
        .map(|request| {
            format!(
                "<div class=\"device\">{}{}{}<strong>{}</strong><input name=\"code\" type=\"text\" \
autocomplete=\"one-time-code\" placeholder=\"{}\" aria-label=\"2FA\" value=\"\"><button type=\"submit\" name=\"approve\" \
value=\"true\">{}</button><button type=\"submit\" name=\"approve\" value=\"false\" class=\"quiet\">{}</button></form></div>",
                form_open("decide-unlock", "row"),
                hidden("requestId", &request.request_id),
                hidden("tab", view.tab.code()),
                escape(request.device_name.as_deref().unwrap_or_default()),
                text(lang, "Code 2FA · facultatif"),
                text(lang, "Autoriser cette demande"),
                text(lang, "Refuser"),
            )
        })
        .collect::<Vec<String>>()
        .concat();
    format!(
        "<section class=\"request-panel\"><h2>{}</h2><p>{}</p>{rows}</section>",
        text(lang, "Demandes de déverrouillage"),
        text(
            lang,
            "Votre accord ouvre le coffre aux clients associés pour le délai configuré. Code 2FA ci-dessus si nécessaire."
        ),
    )
}
