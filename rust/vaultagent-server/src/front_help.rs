//! Page « Aide » : ce que protège le remplissage, comment ajouter un appareil, version du serveur.

use crate::front_escape::escape;
use crate::front_html::View;
use crate::i18n::text;

/// Contenu de la page « Aide ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la section.
#[must_use]
pub fn help(view: &View) -> String {
    let lang = view.lang;
    format!(
        "<section class=\"card single\"><h2>{}</h2><p>{}</p><p>{}</p><p>{}</p><h2>{}</h2><ol><li>{}</li><li>{}</li><li>{}</li></ol>\
<p class=\"note\">{} {}</p></section>",
        text(lang, "Ce que protège le remplissage"),
        text(
            lang,
            "Les mots de passe et codes TOTP sont déchiffrés dans le programme de l’appareil et remplis sans presse-papiers. L’agent reçoit seulement une confirmation."
        ),
        text(
            lang,
            "Un agent qui contrôle entièrement le navigateur peut encore lire un champ rempli. Ce mécanisme évite l’affichage des secrets dans le flux normal ; il ne protège pas contre du code arbitraire malveillant."
        ),
        text(
            lang,
            "Authentik protège l’administration. Chaque appareil possède une autorisation distincte, que vous pouvez retirer. Aucun accès brut aux secrets n’est proposé."
        ),
        text(lang, "Ajouter un appareil"),
        text(
            lang,
            "Ouvrir « Ajouter un appareil », installer le programme et créer un code."
        ),
        text(lang, "Saisir le code dans le programme de l’appareil."),
        text(
            lang,
            "Valider la demande sur la page principale, puis cliquer sur Déverrouiller pour permettre le remplissage."
        ),
        text(lang, "Version du serveur"),
        escape(view.state.session.version),
    )
}
