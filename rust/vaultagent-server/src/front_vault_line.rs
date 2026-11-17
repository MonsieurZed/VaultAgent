//! Ligne du coffre (compte, serveur, verrouillage automatique) et message de la dernière action.

use crate::front_escape::escape;
use crate::front_html::View;
use crate::front_screen::Screen;
use crate::i18n::text;

/// Hôte d'une adresse de coffre : sans schéma, sans chemin.
fn host_of(server: &str) -> &str {
    let rest = server.split_once("://").map_or(server, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// Message de la dernière action, ou le texte vide.
///
/// Paramètres : le contexte de rendu.
/// Retour : le paragraphe du message.
#[must_use]
pub fn message(view: &View) -> String {
    view.notice.map_or_else(String::new, |notice| {
        format!(
            "<p class=\"message\" role=\"status\">{}</p>",
            text(view.lang, notice.label())
        )
    })
}

/// Ligne « <email> sur <hôte> · verrouillage après N min · Modifier ». Un message d'action la remplace.
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la ligne ou du message.
#[must_use]
pub fn line(view: &View) -> String {
    if view.notice.is_some() {
        return message(view);
    }
    let lang = view.lang;
    let Some(account) = &view.state.account else {
        return String::new();
    };
    let locking = account.timeout_minutes.map_or_else(
        || text(lang, "sans verrouillage automatique"),
        |minutes| format!("{} {minutes} {}", text(lang, "verrouillage après"), text(lang, "min")),
    );
    format!(
        "<p class=\"vaultline\">{} {} {} · {locking} · <a href=\"{}\">{}</a></p>",
        escape(&account.email),
        text(lang, "sur"),
        escape(host_of(&account.server)),
        Screen::Vault.href_html(&[]),
        text(lang, "Modifier"),
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
