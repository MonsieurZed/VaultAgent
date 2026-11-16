//! Contexte d'un passage du scénario sur un serveur : adresse, identité, jeton de l'appareil associé et échanges
//! enregistrés ; les helpers construisent les requêtes admin, agent et d'association.

use crate::exchange::{Exchange, from_reply, synthetic};
use crate::http::{Request, send};
use serde_json::{Map, Value};
use std::time::Instant;
use vaultagent_core::device_token::random_uuid;
use vaultagent_core::{RsaPrivateKey, SystemEntropy};

/// Valeurs communes aux deux passages.
pub struct Settings {
    /// Propriétaire attendu par les routes administrateur.
    pub owner: String,
    /// URL `https://` du serveur Bitwarden de test.
    pub server: String,
    /// Courriel du compte factice.
    pub email: String,
    /// Mot de passe maître factice.
    pub password: String,
    /// Suffixe aléatoire des noms de fiches de cette exécution (évite `duplicate-item` entre exécutions).
    pub tag: String,
    /// Clé de l'approbateur de test.
    pub key: RsaPrivateKey,
    /// Clé publique de l'approbateur (SPKI base64).
    pub public_key: String,
}

/// État d'un passage du scénario sur un serveur.
pub struct Run<'a> {
    /// Adresse `hôte:port` du tunnel.
    pub address: String,
    /// Origine HTTPS déclarée du service (`VAULTAGENT_ORIGIN`).
    pub origin: String,
    /// Valeurs communes.
    pub settings: &'a Settings,
    /// En-tête `authorization` de l'appareil A (vide avant l'association).
    pub bearer: String,
    /// Identifiant de coffre rendu par l'association.
    pub vault_id: String,
    /// Échanges enregistrés dans l'ordre.
    pub exchanges: Vec<Exchange>,
    /// Source d'aléa.
    pub entropy: SystemEntropy,
}

/// Construit un objet JSON à partir de paires clé-valeur.
///
/// Paramètres : les paires.
/// Retour : l'objet.
#[must_use]
pub fn object<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

/// Lit un champ texte de premier niveau.
///
/// Paramètres : le corps JSON et la clé.
/// Retour : le texte, ou une chaîne vide s'il manque (le scénario continue et l'écart apparaîtra à la comparaison).
#[must_use]
pub fn field(body: &Value, key: &str) -> String {
    body.get(key).and_then(Value::as_str).unwrap_or_default().to_owned()
}

impl<'a> Run<'a> {
    /// Crée un passage.
    ///
    /// Paramètres : l'adresse du tunnel, l'origine du service et les valeurs communes.
    /// Retour : le contexte vide.
    #[must_use]
    pub fn new(address: &str, origin: &str, settings: &'a Settings) -> Self {
        Self {
            address: address.to_owned(),
            origin: origin.to_owned(),
            settings,
            bearer: String::new(),
            vault_id: String::new(),
            exchanges: Vec::new(),
            entropy: SystemEntropy,
        }
    }

    /// Tire un UUID aléatoire (chaîne vide si l'aléa échoue).
    pub fn uuid(&mut self) -> String {
        random_uuid(&mut self.entropy).unwrap_or_default()
    }

    /// Enregistre un échange synthétique.
    ///
    /// Paramètres : le nom de l'étape et le corps.
    /// Retour : rien.
    pub fn record(&mut self, label: &str, body: Value) {
        self.exchanges.push(synthetic(label, body));
    }

    /// Envoie une requête et enregistre la réponse ; une panne réseau est enregistrée comme un statut 0.
    ///
    /// Paramètres : le nom de l'étape, la méthode, le chemin, les en-têtes et le corps brut.
    /// Retour : le corps JSON de la réponse (chaîne si ce n'est pas du JSON).
    pub fn raw(
        &mut self,
        label: &str,
        method: &str,
        path: &str,
        headers: Vec<(String, String)>,
        body: Option<String>,
    ) -> Value {
        let request = Request {
            method: method.to_owned(),
            path: path.to_owned(),
            headers,
            body,
        };
        let started = Instant::now();
        let exchange = match send(&self.address, &request) {
            Ok(reply) => from_reply(label, &reply),
            Err(error) => Exchange {
                label: label.to_owned(),
                status: 0,
                headers: Vec::new(),
                body: object([("network", Value::from(error.to_string()))]),
            },
        };
        eprintln!("{} {} {}s", self.address, label, started.elapsed().as_secs());
        let body = exchange.body.clone();
        self.exchanges.push(exchange);
        body
    }

    /// Retire de la dernière réponse les fiches `parity-*` (`items` et `trash`) : elles s'accumulent d'un passage à l'autre.
    pub fn drop_parity_items(&mut self) {
        if let Some(last) = self.exchanges.last_mut() {
            for key in ["items", "trash"] {
                if let Some(Value::Array(list)) = last.body.get_mut(key) {
                    list.retain(|entry| !field(entry, "name").starts_with("parity-"));
                }
            }
        }
    }

    /// En-têtes d'une route administrateur `POST`.
    #[must_use]
    pub fn admin_headers(&self) -> Vec<(String, String)> {
        vec![
            ("x-authentik-username".to_owned(), self.settings.owner.clone()),
            ("origin".to_owned(), self.origin.clone()),
            ("content-type".to_owned(), "application/json".to_owned()),
        ]
    }

    /// `GET` administrateur.
    pub fn admin_get(&mut self, label: &str, path: &str) -> Value {
        let headers = vec![("x-authentik-username".to_owned(), self.settings.owner.clone())];
        self.raw(label, "GET", path, headers, None)
    }

    /// `POST /api/admin/<route>` avec un corps JSON.
    pub fn admin_post(&mut self, label: &str, route: &str, body: &Value) -> Value {
        let headers = self.admin_headers();
        self.raw(
            label,
            "POST",
            &format!("/api/admin/{route}"),
            headers,
            Some(body.to_string()),
        )
    }

    /// `POST /api/agent/<route>` avec le jeton de l'appareil A.
    pub fn agent_post(&mut self, label: &str, route: &str, body: &Value) -> Value {
        let headers = vec![
            ("authorization".to_owned(), self.bearer.clone()),
            ("content-type".to_owned(), "application/json".to_owned()),
        ];
        self.raw(
            label,
            "POST",
            &format!("/api/agent/{route}"),
            headers,
            Some(body.to_string()),
        )
    }

    /// `POST /api/pair` avec un corps JSON.
    pub fn pair_post(&mut self, label: &str, body: &Value) -> Value {
        let headers = vec![("content-type".to_owned(), "application/json".to_owned())];
        self.raw(label, "POST", "/api/pair", headers, Some(body.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_and_field_round_trip() {
        let body = object([("a", Value::from("x")), ("b", Value::from(1))]);
        assert_eq!(field(&body, "a"), "x");
        assert_eq!(field(&body, "b"), "");
        assert_eq!(field(&body, "c"), "");
    }
}
