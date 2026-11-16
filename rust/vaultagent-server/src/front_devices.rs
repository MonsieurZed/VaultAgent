//! Liste des appareils associés, avec acceptation automatique et révocation.

use vaultagent_core::PublicDevice;

use crate::front_dates::date_time_of;
use crate::front_escape::escape;
use crate::front_html::{View, form_open, hidden};
use crate::i18n::text;

/// Ligne d'un appareil : nom, date UTC, acceptation automatique et bouton de révocation.
fn row(view: &View, device: &PublicDevice) -> String {
    let lang = view.lang;
    let checked = if device.auto_unlock { " checked" } else { "" };
    let tab = hidden("tab", view.tab.code());
    format!(
        "<div class=\"device\"><div><strong>{}</strong><small>{}</small>{}{}{tab}<input type=\"hidden\" name=\"enabled\" \
value=\"false\"><label class=\"check\"><input type=\"checkbox\" name=\"enabled\" value=\"true\"{checked}><span>{}</span></label>\
<button type=\"submit\" class=\"quiet\">{}</button></form></div>{}{}{tab}<button type=\"submit\" class=\"quiet\">{}</button></form></div>",
        escape(&device.name),
        escape(&date_time_of(&device.created)),
        form_open("auto-unlock", ""),
        hidden("deviceId", &device.id),
        text(lang, "Toujours accepter les demandes de ce client"),
        text(lang, "Enregistrer"),
        form_open("revoke", ""),
        hidden("deviceId", &device.id),
        text(lang, "Révoquer"),
    )
}

/// Liste des appareils, ou le message « Aucun client associé. ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la liste.
#[must_use]
pub fn devices(view: &View) -> String {
    if view.state.devices.is_empty() {
        return format!("<div><p>{}</p></div>", text(view.lang, "Aucun client associé."));
    }
    let rows = view
        .state
        .devices
        .iter()
        .map(|device| row(view, device))
        .collect::<Vec<String>>()
        .concat();
    format!("<div>{rows}</div>")
}
