//! Entrée de `configure` et ses contrôles : serveur, courriel, délai et mot de passe, comme `session.mjs`.

use serde_json::Value;
use vaultagent_core::https_origin;
use zeroize::Zeroizing;

use crate::error::{Result, ServerError};

const MAX_TIMEOUT_MINUTES: u64 = 1440;
const MAX_PASSWORD_UNITS: usize = 8192;

/// Corps de `configure` tel que lu du JSON de la requête.
#[derive(Debug, Clone)]
pub struct ConfigureInput {
    /// Origine du serveur Bitwarden (`https://hôte[:port]`, chemin racine seulement).
    pub server: String,
    /// Courriel du compte.
    pub email: String,
    /// Mot de passe maître ; absent ou vide pour réutiliser celui du compte enregistré.
    pub password: Option<Zeroizing<String>>,
    /// Valeur brute de `timeoutMinutes` : `None` si le champ manque (refusé), `Some(null)` pour désactiver le
    /// délai, sinon un entier de 1 à 1440.
    pub timeout_minutes: Option<Value>,
    /// Code de double authentification facultatif.
    pub code: Option<Zeroizing<String>>,
}

impl ConfigureInput {
    /// Lit le corps JSON de `configure` ; un champ d'un autre type vaut un champ absent, que les contrôles refusent.
    ///
    /// Paramètres : le corps de la requête.
    /// Retour : l'entrée, pas encore validée.
    #[must_use]
    pub fn from_json(body: &Value) -> Self {
        let text = |name: &str| body.get(name).and_then(Value::as_str);
        Self {
            server: text("server").unwrap_or_default().to_owned(),
            email: text("email").unwrap_or_default().to_owned(),
            password: text("password").map(|value| Zeroizing::new(value.to_owned())),
            timeout_minutes: body.get("timeoutMinutes").cloned(),
            code: text("code").map(|value| Zeroizing::new(value.to_owned())),
        }
    }

    /// Valide le serveur et le courriel.
    ///
    /// Paramètres : aucun.
    /// Retour : rien.
    ///
    /// # Errors
    ///
    /// `invalid-account` si le serveur n'est pas une origine HTTPS sans chemin, ou si le courriel n'est pas de
    /// la forme `x@y` sans espace ni second `@`.
    pub fn check_account(&self) -> Result<()> {
        if https_origin(&self.server).is_some() && root_path(&self.server) && plausible_email(&self.email) {
            Ok(())
        } else {
            Err(ServerError::InvalidAccount)
        }
    }

    /// Valide et convertit le délai d'inactivité.
    ///
    /// Paramètres : aucun.
    /// Retour : les minutes, ou `None` pour un délai désactivé.
    ///
    /// # Errors
    ///
    /// `invalid-timeout` si le champ manque ou n'est pas `null` ni un entier de 1 à 1440.
    pub fn timeout(&self) -> Result<Option<u32>> {
        match &self.timeout_minutes {
            Some(Value::Null) => Ok(None),
            Some(Value::Number(number)) => {
                let whole = number.as_u64().or_else(|| {
                    number
                        .as_f64()
                        .filter(|value| value.fract() == 0.0 && *value >= 1.0)
                        .map(float_to_u64)
                });
                whole
                    .filter(|minutes| (1..=MAX_TIMEOUT_MINUTES).contains(minutes))
                    .and_then(|minutes| u32::try_from(minutes).ok())
                    .map(Some)
                    .ok_or(ServerError::InvalidTimeout)
            }
            _ => Err(ServerError::InvalidTimeout),
        }
    }
}

/// Convertit un flottant entier et positif déjà borné par l'appelant.
///
/// Paramètres : le flottant.
/// Retour : l'entier, ou une valeur hors bornes si le flottant est énorme.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn float_to_u64(value: f64) -> u64 {
    if value > 1.0e6 { u64::MAX } else { value as u64 }
}

/// Indique si l'URL n'a pas d'autre chemin que la racine.
///
/// Paramètres : le texte de l'URL, déjà reconnue comme HTTPS.
/// Retour : vrai si rien ne suit l'autorité, hors `/`, requête ou fragment.
fn root_path(server: &str) -> bool {
    let rest = server.strip_prefix("https://").unwrap_or(server);
    match rest.find(['/', '?', '#']) {
        None => true,
        Some(index) => {
            let tail = &rest[index..];
            tail.starts_with(['?', '#'])
                || tail
                    .strip_prefix('/')
                    .is_some_and(|after| after.is_empty() || after.starts_with(['?', '#']))
        }
    }
}

/// Forme `^[^\s@]+@[^\s@]+$`.
///
/// Paramètres : le courriel.
/// Retour : vrai s'il contient exactement un `@`, des deux côtés non vides et sans espace.
fn plausible_email(email: &str) -> bool {
    match email.split_once('@') {
        Some((local, domain)) => {
            let clean = |part: &str| !part.is_empty() && !part.chars().any(|c| c.is_whitespace() || c == '@');
            clean(local) && clean(domain)
        }
        None => false,
    }
}

/// Indique si un mot de passe est utilisable : non vide, 8192 unités UTF-16 au plus.
///
/// Paramètres : le mot de passe.
/// Retour : vrai s'il est accepté.
#[must_use]
pub fn usable_password(password: &str) -> bool {
    !password.is_empty() && password.encode_utf16().count() <= MAX_PASSWORD_UNITS
}
