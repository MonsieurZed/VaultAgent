//! Page « Ajouter un appareil » : nom de l'appareil, création du code d'ajout et téléchargement des programmes.

use crate::front_html::{View, form_open};
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

/// Contenu de la page « Ajouter un appareil ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la section.
#[must_use]
pub fn add(view: &View) -> String {
    let lang = view.lang;
    format!(
        "<section class=\"tab\"><div class=\"section-title\"><h2>{}</h2><p>{}</p></div><h3>{}</h3>{}{}<label><span>{}</span>\
<input name=\"name\" maxlength=\"80\" required placeholder=\"PC personnel / Agent Linux\" value=\"\"></label>\
<button type=\"submit\">{}</button></form></section>",
        text(lang, "Ajouter un appareil"),
        text(
            lang,
            "Installez le programme sur l’appareil utilisé par l’agent, puis créez un code et saisissez-le dans ce programme."
        ),
        text(lang, "Télécharger le programme"),
        downloads(view),
        form_open("pairing", "pair"),
        text(lang, "Nom de l’appareil"),
        text(lang, "Créer un code d’ajout"),
    )
}
