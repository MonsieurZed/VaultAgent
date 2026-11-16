//! Onglet « Clients » : guide, téléchargements, demandes d'association, création d'un code et appareils.

use crate::front_devices::devices;
use crate::front_escape::escape;
use crate::front_html::{View, form_open, hidden};
use crate::i18n::text;

/// Guide « À quoi sert un client ? » avec les liens de téléchargement.
fn guide(view: &View) -> String {
    let lang = view.lang;
    format!(
        "<div class=\"client-guide\"><h3>{}</h3><p>{}</p><ol><li><strong>{}</strong><div class=\"downloads\">\
<a href=\"/downloads/windows-client.zip\">{}</a> <a class=\"checksum\" href=\"/downloads/windows-client.zip.sha256\">SHA256</a>\
<a href=\"/downloads/linux-client.tar.gz\">{}</a> <a class=\"checksum\" href=\"/downloads/linux-client.tar.gz.sha256\">SHA256</a>\
</div></li><li>{}</li><li>{}</li></ol><p>{}</p></div>",
        text(lang, "À quoi sert un client ?"),
        text(
            lang,
            "Ce n’est pas un autre compte du coffre : c’est le petit programme qui remplit les champs là où votre agent travaille."
        ),
        text(lang, "Installer sur l’appareil utilisé par l’agent"),
        text(lang, "Client Windows · sans service"),
        text(lang, "Client Linux · autonome"),
        text(
            lang,
            "Créer un code ci-dessous et le saisir dans le programme du client."
        ),
        text(
            lang,
            "Cliquer sur Autoriser les clients en haut de cette page pour permettre le remplissage."
        ),
        text(
            lang,
            "Le client ne conserve pas votre mot de passe maître. Il garde seulement une autorisation que vous pouvez révoquer."
        ),
    )
}

/// Panneau des demandes d'association ; vide s'il n'y en a aucune.
fn pair_requests(view: &View) -> String {
    if view.state.pair_requests.is_empty() {
        return String::new();
    }
    let lang = view.lang;
    let rows = view
        .state
        .pair_requests
        .iter()
        .map(|request| {
            let approver = if request.approver {
                format!("<br><small>{}</small>", text(lang, "Ce PC deviendra approbateur du coffre."))
            } else {
                String::new()
            };
            format!(
                "<div class=\"device\"><div><strong>{}</strong> <code>{}</code>{approver}</div>{}{}{}\
<button type=\"submit\" name=\"approve\" value=\"true\">{} {}</button><button type=\"submit\" name=\"approve\" value=\"false\" \
class=\"quiet\">{}</button></form></div>",
                escape(&request.name),
                escape(&request.verification),
                form_open("pair-decide", "row"),
                hidden("requestId", &request.request_id),
                hidden("tab", view.tab.code()),
                text(lang, "Autoriser sur"),
                escape(&view.state.vault_name),
                text(lang, "Refuser"),
            )
        })
        .collect::<Vec<String>>()
        .concat();
    format!(
        "<section class=\"request-panel\"><h3>{}</h3><p>{}</p>{rows}</section>",
        text(lang, "Demandes d’association"),
        text(
            lang,
            "Comparez le code avec celui affiché dans le terminal du client avant d’autoriser. Le client sera associé au coffre."
        ),
    )
}

/// Contenu de l'onglet « Clients ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la section.
#[must_use]
pub fn clients(view: &View) -> String {
    let lang = view.lang;
    format!(
        "<section class=\"tab\"><div class=\"section-title\"><h2>{}</h2><p>{}</p></div>{}{}{}<label><span>{}</span>\
<input name=\"name\" maxlength=\"80\" required placeholder=\"PC personnel / Agent Linux\" value=\"\"></label>\
<button type=\"submit\">{}</button></form>{}</section>",
        text(lang, "Vos clients Windows et Linux"),
        text(
            lang,
            "Associez chaque appareil ou agent. Un client peut recevoir une enveloppe chiffrée, jamais ouvrir ou configurer le coffre."
        ),
        guide(view),
        pair_requests(view),
        form_open("pairing", "pair"),
        text(lang, "Nom du client"),
        text(lang, "Créer un code d’association"),
        devices(view),
    )
}
