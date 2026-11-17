//! Bloc « Coffre » : état en grand, champ 2FA avec « Déverrouiller » ou « Verrouiller », résumé du compte et lien
//! « Modifier ».

use crate::front_dates::time_of;
use crate::front_escape::escape;
use crate::front_html::{View, block_title, disabled, form_open};
use crate::front_screen::Screen;
use crate::i18n::text;

/// Hôte d'une adresse de coffre : sans schéma, sans chemin.
fn host_of(server: &str) -> &str {
    let rest = server.split_once("://").map_or(server, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// Classe de couleur et texte de l'état : « À configurer », « Verrouillé » ou « Déverrouillé jusqu'à HH:MM »
/// (heure de Paris).
///
/// Paramètres : le contexte de rendu.
/// Retour : la classe (`setup`, `locked`, `open`) et le libellé échappé.
#[must_use]
pub fn state_of(view: &View) -> (&'static str, String) {
    let (session, lang) = (&view.state.session, view.lang);
    if !session.configured {
        ("setup", text(lang, "À configurer"))
    } else if !session.unlocked {
        ("locked", text(lang, "Verrouillé"))
    } else if let Some(iso) = &session.expires_utc {
        (
            "open",
            format!("{} {}", text(lang, "Déverrouillé jusqu’à"), escape(&time_of(iso))),
        )
    } else {
        ("open", text(lang, "Déverrouillé"))
    }
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

/// Résumé « <email> sur <hôte> · verrouillage après N min · Modifier ».
fn summary(view: &View) -> String {
    let lang = view.lang;
    let Some(account) = &view.state.account else {
        return String::new();
    };
    let locking = account.timeout_minutes.map_or_else(
        || text(lang, "sans verrouillage automatique"),
        |minutes| format!("{} {minutes} {}", text(lang, "verrouillage après"), text(lang, "min")),
    );
    format!(
        "<p class=\"summary\">{} {} {} · {locking} · <a href=\"{}\">{}</a></p>",
        escape(&account.email),
        text(lang, "sur"),
        escape(host_of(&account.server)),
        Screen::Vault.href_html(&[]),
        text(lang, "Modifier"),
    )
}

/// Bloc « Coffre » de la page principale.
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML du bloc.
#[must_use]
pub fn vault_block(view: &View) -> String {
    let (class, label) = state_of(view);
    format!(
        "<section class=\"card b-vault\">{}<p class=\"state {class}\"><strong>{label}</strong></p>{}{}</section>",
        block_title(&text(view.lang, "Coffre"), None),
        control(view),
        summary(view),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_drops_scheme_and_path() {
        assert_eq!(host_of("https://vault.example.test/"), "vault.example.test");
        assert_eq!(
            host_of("https://vault.example.test:8443/a?b"),
            "vault.example.test:8443"
        );
        assert_eq!(host_of("vault.example.test"), "vault.example.test");
    }
}
