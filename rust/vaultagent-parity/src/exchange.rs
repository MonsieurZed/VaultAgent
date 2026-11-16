//! Un échange enregistré : la réponse d'un serveur à une étape du scénario, réduite à ce qui se compare.

use crate::http::Reply;
use serde_json::Value;

const COMPARED_HEADERS: [&str; 3] = ["content-type", "cache-control", "x-content-type-options"];

/// Réponse d'une étape, gardée telle que reçue (les valeurs variables sont normalisées à la comparaison).
#[derive(Debug, Clone)]
pub struct Exchange {
    /// Nom de l'étape, identique sur les deux serveurs.
    pub label: String,
    /// Code de statut.
    pub status: u16,
    /// En-têtes comparés, dans l'ordre de `COMPARED_HEADERS`.
    pub headers: Vec<(String, String)>,
    /// Corps JSON, ou chaîne si le corps n'est pas du JSON.
    pub body: Value,
}

/// Construit l'échange d'une réponse HTTP.
///
/// Paramètres : le nom de l'étape et la réponse.
/// Retour : l'échange, avec les seuls en-têtes comparés.
#[must_use]
pub fn from_reply(label: &str, reply: &Reply) -> Exchange {
    let headers = COMPARED_HEADERS
        .iter()
        .filter_map(|wanted| {
            reply
                .headers
                .iter()
                .find(|(name, _)| name == wanted)
                .map(|(name, value)| (name.clone(), value.clone()))
        })
        .collect();
    let body = serde_json::from_str(&reply.body).unwrap_or_else(|_| Value::String(reply.body.clone()));
    Exchange {
        label: label.to_owned(),
        status: reply.status,
        headers,
        body,
    }
}

/// Construit un échange synthétique (résultat d'une vérification locale, par exemple l'ouverture d'une enveloppe).
///
/// Paramètres : le nom de l'étape et le corps.
/// Retour : l'échange, de statut 200 et sans en-tête.
#[must_use]
pub fn synthetic(label: &str, body: Value) -> Exchange {
    Exchange {
        label: label.to_owned(),
        status: 200,
        headers: Vec::new(),
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_compared_headers_and_parses_json() {
        let reply = Reply {
            status: 200,
            headers: vec![
                ("date".to_owned(), "x".to_owned()),
                ("cache-control".to_owned(), "no-store".to_owned()),
            ],
            body: "{\"alive\":true}".to_owned(),
        };
        let exchange = from_reply("healthz", &reply);
        assert_eq!(
            exchange.headers,
            vec![("cache-control".to_owned(), "no-store".to_owned())]
        );
        assert_eq!(exchange.body.get("alive"), Some(&Value::Bool(true)));
    }

    #[test]
    fn non_json_body_becomes_string() {
        let reply = Reply {
            status: 200,
            headers: vec![],
            body: "<html>".to_owned(),
        };
        assert_eq!(from_reply("x", &reply).body, Value::String("<html>".to_owned()));
    }
}
