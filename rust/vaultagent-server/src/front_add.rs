//! Bloc « Ajouter un appareil » : nom de l'appareil, création du code d'ajout et liens de téléchargement.

use crate::front_html::{View, block_title, form_open};
use crate::i18n::text;

/// Liens de téléchargement Windows et Linux, chacun avec sa somme SHA256.
fn downloads(view: &View) -> String {
    format!(
        "<div class=\"downloads\"><a href=\"/downloads/windows-client.zip\">{}</a> \
<a class=\"checksum\" href=\"/downloads/windows-client.zip.sha256\">SHA256</a>\
<a href=\"/downloads/linux-client.tar.gz\">{}</a> <a class=\"checksum\" href=\"/downloads/linux-client.tar.gz.sha256\">SHA256</a></div>",
        text(view.lang, "Programme Windows · sans service"),
        text(view.lang, "Programme Linux · autonome"),
    )
}

/// Bloc « Ajouter un appareil » de la page principale.
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML du bloc.
#[must_use]
pub fn add(view: &View) -> String {
    let lang = view.lang;
    format!(
        "<section class=\"card b-add\">{}<p>{}</p>{}<label><span>{}</span>\
<input name=\"name\" maxlength=\"80\" required placeholder=\"PC personnel / Agent Linux\" value=\"\"></label>\
<button type=\"submit\">{}</button></form><h3>{}</h3>{}</section>",
        block_title(&text(lang, "Ajouter un appareil"), None),
        text(
            lang,
            "Installez le programme sur l’appareil utilisé par l’agent, puis créez un code et saisissez-le dans ce programme."
        ),
        form_open("pairing", "pair"),
        text(lang, "Nom de l’appareil"),
        text(lang, "Créer un code d’ajout"),
        text(lang, "Télécharger le programme"),
        downloads(view),
    )
}
