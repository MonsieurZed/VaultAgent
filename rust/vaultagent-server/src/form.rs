//! Analyse d'un corps `application/x-www-form-urlencoded` : paires nom-valeur décodées, dans l'ordre reçu.

use crate::query::decode;

/// Champs d'un formulaire.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Form {
    fields: Vec<(String, String)>,
}

impl Form {
    /// Analyse un corps de formulaire (`+` vaut espace, `%XX` est décodé en UTF-8).
    ///
    /// Paramètres : le corps en octets (un corps qui n'est pas de l'UTF-8 est lu avec remplacement).
    /// Retour : le formulaire ; un champ sans `=` a une valeur vide, un champ au nom vide est ignoré.
    #[must_use]
    pub fn parse(body: &[u8]) -> Self {
        let text = String::from_utf8_lossy(body);
        let fields = text
            .split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| pair.split_once('=').unwrap_or((pair, "")))
            .map(|(name, value)| (decode(name), decode(value)))
            .filter(|(name, _)| !name.is_empty())
            .collect();
        Self { fields }
    }

    /// Dernière valeur d'un champ.
    ///
    /// Paramètres : le nom.
    /// Retour : la valeur, ou `None` si le champ manque.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Dernière valeur d'un champ, vide s'il manque.
    ///
    /// Paramètres : le nom.
    /// Retour : la valeur ou le texte vide.
    #[must_use]
    pub fn text(&self, name: &str) -> &str {
        self.get(name).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_decoded_in_order() {
        let form = Form::parse(b"name=Poste+%C3%A9t%C3%A9&code=&flag&x=1&x=2&=skipped");
        assert_eq!(form.get("name"), Some("Poste été"));
        assert_eq!((form.get("code"), form.get("flag")), (Some(""), Some("")));
        assert_eq!(form.get("x"), Some("2"));
        assert_eq!(form.get("absent"), None);
        assert_eq!(form.text("absent"), "");
    }

    #[test]
    fn reserved_characters_come_back_exactly() {
        let form = Form::parse(b"a=%26%3D%25+b&b=%zz");
        assert_eq!((form.text("a"), form.text("b")), ("&=% b", "%zz"));
    }

    #[test]
    fn an_empty_body_has_no_field() {
        assert_eq!(Form::parse(b""), Form::default());
    }
}
