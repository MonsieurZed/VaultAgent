//! Bloc « Appareils » : une ligne par appareil, avec son rôle, sa date d'ajout, « Sans demander » pour un agent et
//! « Retirer » avec confirmation sur la ligne, et pagination.

use vaultagent_core::PublicDevice;

use crate::device_kind::DeviceKind;
use crate::front_dates::date_of;
use crate::front_escape::escape;
use crate::front_html::{View, block_title, form_open, hidden};
use crate::front_paging::Paging;
use crate::front_screen::Screen;
use crate::i18n::text;

/// Rôle affiché : « agent », ou « validateur · UAC | Hello | carte ».
fn role(view: &View, kind: DeviceKind) -> String {
    let lang = view.lang;
    let mode = match kind {
        DeviceKind::Agent => return text(lang, "agent"),
        DeviceKind::Card => text(lang, "carte"),
        DeviceKind::Elevation => "UAC".to_owned(),
        DeviceKind::Hello => "Hello".to_owned(),
    };
    format!("{} · {mode}", text(lang, "validateur"))
}

/// Formulaire « Sans demander » d'un agent, avec le champ caché qui donne la valeur décochée.
fn auto_form(view: &View, device: &PublicDevice) -> String {
    let checked = if device.auto_unlock { " checked" } else { "" };
    format!(
        "{}{}<input type=\"hidden\" name=\"enabled\" value=\"false\"><label class=\"check\"><input type=\"checkbox\" \
name=\"enabled\" value=\"true\"{checked}><span>{}</span></label><button type=\"submit\" class=\"quiet\">{}</button></form>",
        form_open("auto-unlock", "inline"),
        hidden("deviceId", &device.id),
        text(view.lang, "Sans demander"),
        text(view.lang, "Enregistrer"),
    )
}

/// Texte encodé pour une valeur de paramètre d'adresse.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// Adresse de la page principale à la page donnée, avec des paramètres supplémentaires.
fn main_href(page: usize, extra: &[(&str, &str)]) -> String {
    let page_text = page.to_string();
    let mut params: Vec<(&str, &str)> = Vec::new();
    if page > 1 {
        params.push(("page", &page_text));
    }
    params.extend_from_slice(extra);
    Screen::Main.href_html(&params)
}

/// Ligne d'un appareil dont le retrait attend sa confirmation : avertissement, « Confirmer le retrait », « Annuler ».
fn confirm_row(view: &View, device: &PublicDevice, paging: &Paging) -> String {
    let lang = view.lang;
    format!(
        "<div class=\"device confirm\"><div><strong>{}</strong> <small class=\"danger\">{}</small></div>{}{}<button type=\"submit\" class=\"danger-button\">{}</button></form><a href=\"{}\">{}</a></div>",
        escape(&device.name),
        text(lang, "Il ne pourra plus rien remplir."),
        form_open("revoke", "inline"),
        hidden("deviceId", &device.id),
        text(lang, "Confirmer le retrait"),
        main_href(paging.page, &[]),
        text(lang, "Annuler"),
    )
}

/// Ligne d'un appareil.
fn row(view: &View, device: &PublicDevice, paging: &Paging) -> String {
    if view.confirm.as_deref() == Some(device.id.as_str()) {
        return confirm_row(view, device, paging);
    }
    let kind = view
        .state
        .device_kinds
        .get(&device.id)
        .copied()
        .unwrap_or(DeviceKind::Agent);
    let auto = if kind.is_validator() {
        String::new()
    } else {
        auto_form(view, device)
    };
    format!(
        "<div class=\"device\"><div><strong>{}</strong> <small>{}</small><small>{} {}</small></div>{auto}<a class=\"button quiet danger\" href=\"{}\">{}</a></div>",
        escape(&device.name),
        role(view, kind),
        text(view.lang, "ajouté le"),
        escape(&date_of(&device.created)),
        main_href(paging.page, &[("confirm", &encode(&device.id))]),
        text(view.lang, "Retirer"),
    )
}

/// Liens « Précédent », « N / M » et « Suivant » ; absents quand il n'y a qu'une page.
fn pager(view: &View, paging: &Paging) -> String {
    if paging.count < 2 {
        return String::new();
    }
    let lang = view.lang;
    let previous = if paging.page > 1 {
        format!(
            "<a href=\"{}\">‹ {}</a>",
            main_href(paging.page - 1, &[]),
            text(lang, "Précédent")
        )
    } else {
        String::new()
    };
    let next = if paging.page < paging.count {
        format!(
            "<a href=\"{}\">{} ›</a>",
            main_href(paging.page + 1, &[]),
            text(lang, "Suivant")
        )
    } else {
        String::new()
    };
    format!(
        "<nav class=\"pager\">{previous}<span>{} / {}</span>{next}</nav>",
        paging.page, paging.count
    )
}

/// Bloc « Appareils » avec son compteur et sa pagination.
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML du bloc.
#[must_use]
pub fn devices(view: &View) -> String {
    let lang = view.lang;
    let all = &view.state.devices;
    let paging = Paging::new(all.len(), view.page.as_deref());
    let rows = if all.is_empty() {
        format!("<p class=\"empty\">{}</p>", text(lang, "Aucun appareil."))
    } else {
        all[paging.range(all.len())]
            .iter()
            .map(|device| row(view, device, &paging))
            .collect::<Vec<String>>()
            .concat()
    };
    format!(
        "<section class=\"card b-devices\">{}<div class=\"devices\">{rows}</div>{}</section>",
        block_title(&text(lang, "Appareils"), Some(all.len())),
        pager(view, &paging),
    )
}
