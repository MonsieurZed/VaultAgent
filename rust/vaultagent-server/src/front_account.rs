//! Mode « Modifier » (`?view=vault`) : le formulaire de connexion au coffre remplace le contenu du bloc Coffre ; les
//! champs secrets sont toujours rendus vides.

use crate::front_escape::escape;
use crate::front_html::{View, form_open};
use crate::front_vault_block::state_of;
use crate::i18n::text;

const DEFAULT_MINUTES: u32 = 15;

/// Champ du formulaire avec son libellé et son aide.
fn field(view: &View, label: &'static str, help: &'static str, input: &str) -> String {
    format!(
        "<label><span>{}</span>{input}<small>{}</small></label>",
        text(view.lang, label),
        text(view.lang, help)
    )
}

/// Lien « Annuler » vers la page principale, absent tant que le coffre n'est pas configuré.
fn cancel(view: &View) -> String {
    if view.state.session.configured {
        format!(
            "<a class=\"button quiet\" href=\"/\">{}</a>",
            text(view.lang, "Annuler")
        )
    } else {
        String::new()
    }
}

/// Bloc « Coffre » en mode « Modifier ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML du bloc.
#[must_use]
pub fn account(view: &View) -> String {
    let lang = view.lang;
    let (server, email, minutes, automatic) = match &view.state.account {
        Some(account) => (
            account.server.as_str(),
            account.email.as_str(),
            account.timeout_minutes.unwrap_or(DEFAULT_MINUTES),
            account.timeout_minutes.is_some(),
        ),
        None => ("", "", DEFAULT_MINUTES, true),
    };
    let fields = [
        field(
            view,
            "Adresse HTTPS du coffre",
            "L’adresse que vous ouvrez pour accéder à Bitwarden ou Vaultwarden.",
            &format!(
                "<input name=\"server\" type=\"url\" required placeholder=\"https://vault.example.com\" value=\"{}\">",
                escape(server)
            ),
        ),
        field(
            view,
            "Adresse email",
            "Le compte de votre coffre, pas nécessairement celui du SSO.",
            &format!(
                "<input name=\"email\" type=\"email\" required autocomplete=\"username\" value=\"{}\">",
                escape(email)
            ),
        ),
        field(
            view,
            "Mot de passe maître",
            "Déjà enregistré ? Laissez vide pour le conserver. Ne le transmettez pas dans le chat.",
            "<input name=\"password\" type=\"password\" autocomplete=\"new-password\" value=\"\">",
        ),
        field(
            view,
            "Code 2FA · facultatif",
            "Seulement si la double authentification est activée sur ce coffre.",
            "<input name=\"code\" autocomplete=\"one-time-code\" value=\"\">",
        ),
    ]
    .concat();
    let checked = if automatic { " checked" } else { "" };
    let (state_class, state_label) = state_of(view);
    format!(
        "<section class=\"card b-vault\"><h2>{} <span class=\"chip {state_class}\">{state_label}</span></h2><p>{}</p>{}\
<div class=\"grid\">{fields}</div>\
<fieldset><legend>{}</legend><label class=\"check\"><input name=\"automatic\" type=\"checkbox\" value=\"on\"{checked}>\
<span>{}</span></label><label class=\"timeout\"><span>{}</span><input name=\"minutes\" type=\"number\" min=\"1\" max=\"1440\" \
value=\"{minutes}\"></label><p>{}</p></fieldset><div class=\"submit-row\"><button type=\"submit\">{}</button>{}\
<p class=\"note\">{}</p></div></form></section>",
        text(lang, "Connecter votre coffre"),
        text(
            lang,
            "À configurer une fois. Le mot de passe n’est jamais renvoyé par l’API."
        ),
        form_open("configure", "account"),
        text(lang, "Après utilisation"),
        text(lang, "Verrouiller automatiquement"),
        text(lang, "Délai en minutes"),
        text(
            lang,
            "Chaque utilisation autorisée prolonge le délai. Le redémarrage du serveur verrouille toujours le coffre."
        ),
        text(lang, "Vérifier et enregistrer"),
        cancel(view),
        text(lang, "Changer de compte retire tous les appareils."),
    )
}
