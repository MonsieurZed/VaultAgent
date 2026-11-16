//! Onglet « Compte » : formulaire de connexion au coffre ; les champs secrets sont toujours rendus vides.

use crate::front_escape::escape;
use crate::front_html::{View, form_open};
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

/// Contenu de l'onglet « Compte ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la section.
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
    format!(
        "<section class=\"tab\"><div class=\"section-title\"><h2>{}</h2><p>{}</p></div>{}<div class=\"grid\">{fields}</div>\
<fieldset><legend>{}</legend><label class=\"check\"><input name=\"automatic\" type=\"checkbox\" value=\"on\"{checked}>\
<span>{}</span></label><label class=\"timeout\"><span>{}</span><input name=\"minutes\" type=\"number\" min=\"1\" max=\"1440\" \
value=\"{minutes}\"></label><p>{}</p></fieldset><button type=\"submit\">{}</button><p class=\"note\">{}</p></form></section>",
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
        text(
            lang,
            "Changer de compte révoque les clients associés. Windows Hello et les passkeys Windows ne sont pas fournis par ce serveur."
        ),
    )
}
