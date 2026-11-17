//! Traduction d'un formulaire du front en corps JSON de la route d'administration correspondante.

use serde_json::{Map, Number, Value};

use crate::form::Form;
use crate::front_screen::Screen;

/// Corps JSON et écran de retour d'une action de formulaire.
pub struct Prepared {
    /// Nom de l'action d'administration.
    pub action: &'static str,
    /// Corps JSON passé à la même fonction que la route JSON.
    pub body: Value,
    /// Écran de la redirection.
    pub screen: Screen,
}

/// Objet JSON à partir de paires nom-valeur.
fn object(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect::<Map<_, _>>(),
    )
}

/// Chaîne JSON.
fn string(text: &str) -> Value {
    Value::String(text.to_owned())
}

/// Écran d'un champ caché `view`, la page principale à défaut.
fn view_field(form: &Form) -> Screen {
    Screen::parse(form.text("view")).unwrap_or(Screen::Main)
}

/// Délai de `configure` : désactivé si la case manque, sinon un nombre (un texte non numérique reste texte, la route le
/// refuse).
fn timeout(form: &Form) -> Value {
    if form.get("automatic") != Some("on") {
        return Value::Null;
    }
    let minutes = form.text("minutes").trim();
    minutes
        .parse::<u64>()
        .map_or_else(|_| string(minutes), |value| Value::Number(Number::from(value)))
}

/// Prépare une action de formulaire.
///
/// Paramètres : le nom de l'action (`configure`, `unlock`, `lock`, `pairing`, `pair-decide`, `decide-unlock`,
/// `auto-unlock`, `revoke`), le formulaire.
/// Retour : l'action préparée, ou `None` pour un nom hors de la liste.
#[must_use]
pub fn prepare(name: &str, form: &Form) -> Option<Prepared> {
    let approve = form.get("approve") == Some("true");
    let (action, body, screen) = match name {
        "configure" => (
            "configure",
            object(vec![
                ("server", string(form.text("server").trim().trim_end_matches('/'))),
                ("email", string(form.text("email").trim())),
                ("password", string(form.text("password"))),
                ("code", string(form.text("code"))),
                ("timeoutMinutes", timeout(form)),
            ]),
            Screen::Vault,
        ),
        "unlock" => (
            "unlock",
            object(vec![("code", string(form.text("code")))]),
            view_field(form),
        ),
        "lock" => ("lock", object(Vec::new()), view_field(form)),
        "pairing" => (
            "pairing",
            object(vec![("name", string(form.text("name")))]),
            Screen::Main,
        ),
        "pair-decide" => (
            "pair-decide",
            object(vec![
                ("requestId", string(form.text("requestId"))),
                ("approve", Value::Bool(approve)),
            ]),
            Screen::Main,
        ),
        "decide-unlock" => {
            let mut pairs = vec![
                ("requestId", string(form.text("requestId"))),
                ("approve", Value::Bool(approve)),
            ];
            if approve {
                pairs.push(("code", string(form.text("code"))));
            }
            ("decide-unlock", object(pairs), view_field(form))
        }
        "auto-unlock" => (
            "auto-unlock",
            object(vec![
                ("deviceId", string(form.text("deviceId"))),
                ("enabled", Value::Bool(form.get("enabled") == Some("true"))),
            ]),
            Screen::Main,
        ),
        "revoke" => (
            "revoke",
            object(vec![("deviceId", string(form.text("deviceId")))]),
            Screen::Main,
        ),
        _ => return None,
    };
    Some(Prepared { action, body, screen })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_trims_and_disables_the_timeout_without_the_box() {
        let form = Form::parse(b"server=+https%3A%2F%2Fvault.example.test%2F&email=a%40b.c&password=x&minutes=20");
        let prepared = prepare("configure", &form).unwrap();
        assert_eq!(prepared.body["server"], "https://vault.example.test");
        assert_eq!(prepared.body["timeoutMinutes"], Value::Null);
        let checked = Form::parse(b"automatic=on&minutes=20");
        assert_eq!(prepare("configure", &checked).unwrap().body["timeoutMinutes"], 20);
        let garbage = Form::parse(b"automatic=on&minutes=abc");
        assert_eq!(prepare("configure", &garbage).unwrap().body["timeoutMinutes"], "abc");
    }

    #[test]
    fn decisions_read_the_clicked_button() {
        let form = Form::parse(b"requestId=r1&approve=true&code=123456&view=help");
        let prepared = prepare("decide-unlock", &form).unwrap();
        assert_eq!(
            (prepared.body["approve"].clone(), prepared.body["code"].clone()),
            (Value::Bool(true), "123456".into())
        );
        assert_eq!(prepared.screen, Screen::Help);
        let denied = prepare("decide-unlock", &Form::parse(b"requestId=r1&approve=false&code=1")).unwrap();
        assert!(denied.body.get("code").is_none());
    }

    #[test]
    fn the_automatic_box_uses_the_last_value() {
        let form = Form::parse(b"deviceId=d&enabled=false&enabled=true");
        assert_eq!(
            prepare("auto-unlock", &form).unwrap().body["enabled"],
            Value::Bool(true)
        );
        let off = Form::parse(b"deviceId=d&enabled=false");
        assert_eq!(
            prepare("auto-unlock", &off).unwrap().body["enabled"],
            Value::Bool(false)
        );
    }

    #[test]
    fn an_unknown_action_is_not_prepared() {
        assert!(prepare("write", &Form::default()).is_none());
    }
}
