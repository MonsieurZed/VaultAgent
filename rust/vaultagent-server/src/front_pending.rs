//! Section « À valider » : demandes de déverrouillage et demandes d'ajout d'appareil ; absente quand rien n'attend.

use crate::front_escape::escape;
use crate::front_html::{View, form_open, hidden};
use crate::i18n::text;

/// Indique si au moins une demande attend une décision.
///
/// Paramètres : le contexte de rendu.
/// Retour : `true` si une demande de déverrouillage ou d'ajout attend.
#[must_use]
pub fn has_pending(view: &View) -> bool {
    !view.state.unlock_requests.is_empty() || !view.state.pair_requests.is_empty()
}

/// Nombre de lignes de demandes affichées.
///
/// Paramètres : le contexte de rendu.
/// Retour : les demandes de déverrouillage plus les demandes d'ajout.
#[must_use]
pub fn request_rows(view: &View) -> usize {
    view.state.unlock_requests.len() + view.state.pair_requests.len()
}

/// Lignes des demandes de déverrouillage : appareil, champ 2FA, « Déverrouiller » et « Refuser ».
fn unlock_rows(view: &View) -> String {
    let lang = view.lang;
    view.state
        .unlock_requests
        .iter()
        .map(|request| {
            format!(
                "<div class=\"device\">{}{}<strong>{}</strong><small>{}</small><input name=\"code\" type=\"text\" \
autocomplete=\"one-time-code\" placeholder=\"{}\" aria-label=\"2FA\" value=\"\"><button type=\"submit\" name=\"approve\" \
value=\"true\">{}</button><button type=\"submit\" name=\"approve\" value=\"false\" class=\"quiet\">{}</button></form></div>",
                form_open("decide-unlock", "row"),
                hidden("requestId", &request.request_id),
                escape(request.device_name.as_deref().unwrap_or_default()),
                text(lang, "Demande de déverrouillage"),
                text(lang, "Code 2FA · facultatif"),
                text(lang, "Déverrouiller"),
                text(lang, "Refuser"),
            )
        })
        .collect::<Vec<String>>()
        .concat()
}

/// Lignes des demandes d'ajout : nom, code à comparer, rôle demandé, « Ajouter » et « Refuser ».
fn pair_rows(view: &View) -> String {
    let lang = view.lang;
    view.state
        .pair_requests
        .iter()
        .map(|request| {
            let role = if request.approver {
                format!(" <small>{}</small>", text(lang, "validateur"))
            } else {
                String::new()
            };
            format!(
                "<div class=\"device\">{}{}<strong>{}</strong>{role}<small>{} <code>{}</code></small>\
<button type=\"submit\" name=\"approve\" value=\"true\">{}</button><button type=\"submit\" name=\"approve\" \
value=\"false\" class=\"quiet\">{}</button></form></div>",
                form_open("pair-decide", "row"),
                hidden("requestId", &request.request_id),
                escape(&request.name),
                text(lang, "Code à comparer"),
                escape(&request.verification),
                text(lang, "Ajouter"),
                text(lang, "Refuser"),
            )
        })
        .collect::<Vec<String>>()
        .concat()
}

/// Section « À valider ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la section, ou le texte vide si aucune demande n'attend.
#[must_use]
pub fn pending(view: &View) -> String {
    if !has_pending(view) {
        return String::new();
    }
    format!(
        "<section class=\"request-panel\"><h2>{}</h2>{}{}</section>",
        text(view.lang, "À valider"),
        unlock_rows(view),
        pair_rows(view),
    )
}
