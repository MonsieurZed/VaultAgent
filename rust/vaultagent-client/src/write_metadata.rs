//! Métadonnées d'une écriture : objet JSON sans secret, clés admises selon l'action.

use serde_json::{Map, Value};

use crate::error::ClientError;

/// Clés admises à la création d'une fiche.
const CREATE_KEYS: [&str; 6] = ["type", "name", "folderId", "login", "organizationId", "collectionIds"];

/// Clés admises à la mise à jour d'une fiche.
const UPDATE_KEYS: [&str; 4] = ["name", "folderId", "removeUris", "addUris"];

/// Métadonnées validées : le reste des clés et, à la création, l'objet `login` (`uris` seulement).
#[derive(Debug, Default)]
pub struct Metadata {
    /// Clés autres que `login`.
    pub rest: Map<String, Value>,
    /// Contenu de `login` s'il était fourni.
    pub login: Option<Map<String, Value>>,
}

impl Metadata {
    /// Indique si le type de la fiche est 1 (identifiant).
    #[must_use]
    pub fn is_login_type(&self) -> bool {
        self.rest.get("type").and_then(Value::as_i64) == Some(1)
    }
}

/// Erreur commune : un secret ou une clé inconnue ne passe jamais par les métadonnées.
fn refused() -> ClientError {
    ClientError::Usage("Invalid metadata: secrets belong in --values-stdin.".to_owned())
}

/// Analyse un objet JSON et contrôle ses clés.
fn parse(text: &str, allowed: &[&str]) -> Result<Metadata, ClientError> {
    let value: Value =
        serde_json::from_str(text).map_err(|_| ClientError::Usage("Metadata must be a JSON object.".to_owned()))?;
    let Value::Object(mut map) = value else {
        return Err(ClientError::Usage("Metadata must be a JSON object.".to_owned()));
    };
    if map.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(refused());
    }
    let login = match map.remove("login") {
        None => None,
        Some(Value::Object(login)) if login.keys().all(|key| key == "uris") => Some(login),
        Some(_) => return Err(refused()),
    };
    Ok(Metadata { rest: map, login })
}

/// Valide les métadonnées d'une création.
///
/// Paramètres : le texte JSON de `--metadata`.
/// Retour : les métadonnées, ou `Usage` pour un texte invalide, une clé hors liste ou un `login` portant autre
/// chose que `uris` (message : « secrets belong in --values-stdin »).
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn parse_create(text: &str) -> Result<Metadata, ClientError> {
    parse(text, &CREATE_KEYS)
}

/// Valide les métadonnées d'une mise à jour.
///
/// Paramètres : le texte JSON de `--metadata`.
/// Retour : les métadonnées, ou `Usage` pour un texte invalide ou une clé hors liste (`login` compris).
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn parse_update(text: &str) -> Result<Metadata, ClientError> {
    parse(text, &UPDATE_KEYS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_accepts_the_allowed_keys_and_splits_login() {
        let meta = parse_create("{\"type\":1,\"name\":\"n\",\"login\":{\"uris\":[]},\"folderId\":null}").unwrap();
        assert!(meta.is_login_type());
        assert!(meta.login.is_some());
        assert!(!meta.rest.contains_key("login"));
    }

    #[test]
    fn secrets_and_unknown_keys_are_refused() {
        for bad in [
            "{\"type\":1,\"notes\":\"x\"}",
            "{\"login\":{\"password\":\"x\"}}",
            "{\"login\":\"x\"}",
            "{\"password\":\"x\"}",
            "[]",
            "nope",
        ] {
            assert_eq!(parse_create(bad).unwrap_err().code(), "usage", "{bad}");
        }
        assert!(
            parse_create("{\"password\":\"x\"}")
                .unwrap_err()
                .message()
                .contains("--values-stdin")
        );
    }

    #[test]
    fn update_has_its_own_key_list() {
        assert!(parse_update("{\"name\":\"n\",\"addUris\":[],\"removeUris\":[],\"folderId\":\"f\"}").is_ok());
        assert!(parse_update("{\"type\":1}").is_err());
        assert!(parse_update("{\"login\":{\"uris\":[]}}").is_err());
    }
}
