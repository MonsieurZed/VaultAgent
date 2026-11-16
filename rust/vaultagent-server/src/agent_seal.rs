//! Route `seal` : met sous enveloppe RSA-OAEP et AES-GCM un champ d'une fiche pour la clé publique du client.

use serde_json::Value;
use vaultagent_core::{ErrorCode, Uuid, https_origin, seal};

use crate::context::Context;
use crate::error::Result;
use crate::guard::Guard;
use crate::http::Response;

const FIELDS: [&str; 3] = ["username", "password", "totp"];
const VALUE_LIMIT: usize = 65_536;
const DEFAULT_PERIOD: f64 = 30.0;
const WAIT_BELOW_MS: u64 = 5_000;
const WAIT_MARGIN_MS: u64 = 100;

/// Corps validé de `seal`.
struct SealInput<'a> {
    item_id: Uuid,
    field: &'a str,
    origin: &'a str,
    public_key: &'a str,
}

/// Valide le corps de `seal`.
fn parse(body: &Value) -> Result<SealInput<'_>> {
    let text = |key: &str| body.get(key).and_then(Value::as_str);
    let item_id = text("itemId").and_then(Uuid::parse);
    let field = text("field").filter(|field| FIELDS.contains(field));
    let origin = text("origin").filter(|origin| https_origin(origin).as_deref() == Some(*origin));
    match (item_id, field, origin) {
        (Some(item_id), Some(field), Some(origin)) => Ok(SealInput {
            item_id,
            field,
            origin,
            public_key: text("publicKey").unwrap_or_default(),
        }),
        _ => Err(ErrorCode::InvalidField.into()),
    }
}

/// Période en secondes d'une URI `otpauth://…?period=N` ; 30 pour une graine simple ou une valeur invalide.
fn totp_period(seed: &str) -> f64 {
    let is_uri = seed.split_once("://").is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"+-.".contains(&byte))
    });
    let query = seed
        .split_once('?')
        .map(|(_, rest)| rest.split('#').next().unwrap_or_default());
    let value = is_uri
        .then_some(query)
        .flatten()
        .and_then(|query| query.split('&').find_map(|pair| pair.strip_prefix("period=")))
        .and_then(|text| text.trim().parse::<f64>().ok())
        .filter(|number| *number != 0.0 && number.is_finite());
    value.unwrap_or(DEFAULT_PERIOD)
}

/// Attend la période TOTP suivante quand il reste moins de 5 secondes dans la courante.
fn wait_for_period(context: &Context, period: f64) {
    let period_ms = (period * 1000.0).round();
    let now = context.clock.wall_ms();
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let remaining = (period_ms - (now as f64) % period_ms) as u64;
    if remaining < WAIT_BELOW_MS {
        context.pause.sleep_ms(remaining + WAIT_MARGIN_MS);
    }
}

/// `seal` : une enveloppe chiffrée remplace toute valeur de champ.
///
/// Paramètres : le contexte, la garde, le corps.
/// Retour : l'enveloppe `{encryptedKey, nonce, tag, data}`.
///
/// # Errors
///
/// `invalid-field`, `origin-refused`, `totp-period`, `field-missing`, `invalid-key`, `locked`, `unauthorized` ou
/// l'erreur du moteur.
pub fn seal_field(context: &Context, guard: &Guard<'_>, body: &Value) -> Result<Response> {
    guard.authorize()?;
    let input = parse(body)?;
    guard.slot()?;
    guard.sync()?;
    let item = guard.call(|engine| engine.item(&input.item_id))?;
    let login = item.login.as_ref();
    let allowed = login.is_some_and(|login| {
        login
            .uris
            .iter()
            .any(|entry| https_origin(&entry.uri).as_deref() == Some(input.origin))
    });
    if !allowed {
        return Err(ErrorCode::OriginRefused.into());
    }
    let value = match input.field {
        "totp" => {
            let seed = login
                .and_then(|login| login.totp.as_ref())
                .map(|seed| seed.expose().to_owned());
            let period = totp_period(seed.as_deref().unwrap_or_default());
            if !(15.0..=120.0).contains(&period) {
                return Err(ErrorCode::TotpPeriod.into());
            }
            wait_for_period(context, period);
            guard.slot()?;
            Some(guard.call(|engine| engine.totp(&input.item_id))?.expose().to_owned())
        }
        "username" => login.and_then(|login| login.username.clone()),
        _ => login
            .and_then(|login| login.password.as_ref())
            .map(|secret| secret.expose().to_owned()),
    };
    let value =
        zeroize::Zeroizing::new(value.filter(|text| !text.is_empty() && text.encode_utf16().count() <= VALUE_LIMIT));
    let Some(clear) = value.as_ref() else {
        return Err(ErrorCode::FieldMissing.into());
    };
    guard.check()?;
    let envelope = seal(clear.as_bytes(), input.public_key, &mut context.entropy.clone())?;
    guard.touch();
    Ok(Response::json(200, &envelope))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_comes_from_an_otpauth_uri_only() {
        assert!((totp_period("otpauth://totp/x?secret=A&period=60") - 60.0).abs() < f64::EPSILON);
        assert!((totp_period("FAKESEED") - 30.0).abs() < f64::EPSILON);
        assert!((totp_period("ABC?period=60") - 30.0).abs() < f64::EPSILON);
        assert!((totp_period("otpauth://totp/x?period=abc") - 30.0).abs() < f64::EPSILON);
        assert!((totp_period("otpauth://totp/x?period=0") - 30.0).abs() < f64::EPSILON);
    }
}
