//! Onglet « Fonctionnement » : ce que protège le remplissage.

use crate::front_html::View;
use crate::i18n::text;

/// Contenu de l'onglet « Fonctionnement ».
///
/// Paramètres : le contexte de rendu.
/// Retour : le HTML de la section.
#[must_use]
pub fn about(view: &View) -> String {
    let lang = view.lang;
    format!(
        "<section class=\"tab\"><h2>{}</h2><p>{}</p><p>{}</p><p>{}</p></section>",
        text(lang, "Ce que protège le remplissage"),
        text(
            lang,
            "Les mots de passe et codes TOTP sont déchiffrés dans le helper du client et remplis sans presse-papiers. L’agent reçoit seulement une confirmation."
        ),
        text(
            lang,
            "Un agent qui contrôle entièrement le navigateur peut encore lire un champ rempli. Ce mécanisme évite l’affichage des secrets dans le flux normal ; il ne protège pas contre du code arbitraire malveillant."
        ),
        text(
            lang,
            "Authentik protège l’administration. Chaque client possède une autorisation distincte et révocable. Aucun accès brut aux secrets n’est proposé."
        ),
    )
}
