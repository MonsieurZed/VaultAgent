//! Échappement HTML : toute donnée insérée dans une page passe par [`escape`].

/// Remplace les cinq caractères qui ont un sens en HTML (`&`, `<`, `>`, `"`, `'`) par leur entité.
///
/// Paramètres : le texte brut.
/// Retour : le texte sûr pour un contenu d'élément ou une valeur d'attribut entre guillemets.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_special_characters_are_replaced() {
        assert_eq!(
            escape("<a href=\"x\">'&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;"
        );
    }

    #[test]
    fn plain_and_accented_text_is_unchanged() {
        assert_eq!(escape("Compte · é’"), "Compte · é’");
    }
}
