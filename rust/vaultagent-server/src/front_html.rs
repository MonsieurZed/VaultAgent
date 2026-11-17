//! Éléments HTML élémentaires du front et contexte de rendu partagé par tous les organismes de la page.

use crate::admin_state::State;
use crate::front_escape::escape;
use crate::front_lang::Lang;
use crate::front_notice::Notice;
use crate::front_screen::Screen;

/// Contexte d'un rendu : langue, écran, message, page, confirmation et état du coffre.
pub struct View<'a> {
    /// Langue d'affichage.
    pub lang: Lang,
    /// Écran affiché.
    pub screen: Screen,
    /// Message de la dernière action.
    pub notice: Option<Notice>,
    /// Paramètre `page` brut de la liste des appareils.
    pub page: Option<String>,
    /// Appareil dont le retrait attend sa confirmation (identifiant d'un appareil connu seulement).
    pub confirm: Option<String>,
    /// État administrateur du coffre.
    pub state: &'a State,
}

/// Champ caché.
///
/// Paramètres : le nom, la valeur (échappée ici).
/// Retour : l'élément `<input type="hidden">`.
#[must_use]
pub fn hidden(name: &str, value: &str) -> String {
    format!(
        "<input type=\"hidden\" name=\"{}\" value=\"{}\">",
        escape(name),
        escape(value)
    )
}

/// Ouvre un formulaire `POST` vers une action d'administration.
///
/// Paramètres : le nom de l'action, l'attribut de classe éventuel (déjà sûr, sans guillemet).
/// Retour : la balise ouvrante.
#[must_use]
pub fn form_open(action: &str, class: &str) -> String {
    let class = if class.is_empty() {
        String::new()
    } else {
        format!(" class=\"{class}\"")
    };
    format!("<form method=\"post\" action=\"/admin/{}\"{class}>", escape(action))
}

/// Attribut `disabled` ou rien.
///
/// Paramètres : l'élément est-il désactivé.
/// Retour : ` disabled` ou le texte vide.
#[must_use]
pub fn disabled(is_disabled: bool) -> &'static str {
    if is_disabled { " disabled" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hidden_field_escapes_its_value() {
        assert_eq!(
            hidden("id", "a\"><script>"),
            "<input type=\"hidden\" name=\"id\" value=\"a&quot;&gt;&lt;script&gt;\">"
        );
    }

    #[test]
    fn a_form_posts_to_the_admin_action() {
        assert_eq!(form_open("lock", ""), "<form method=\"post\" action=\"/admin/lock\">");
        assert!(form_open("lock", "inline").ends_with("class=\"inline\">"));
    }
}
