//! Formes JSON des sorties de saisie directe : fenêtre décrite, refus de cible, comptes rendus de `type` et `focus`.

use serde::Serialize;

use crate::error::ClientError;

/// Identité visible d'une fenêtre dans un rapport ; jamais de handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowDescription {
    /// Nom du processus.
    pub process: String,
    /// Titre de la fenêtre, ou fragment exigé pour la cible attendue.
    pub title: String,
}

/// Refus de saisie ou de mise au premier plan : raison, cible attendue et fenêtre active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetRefusal {
    /// Phrase courte expliquant le refus, sans secret.
    pub reason: String,
    /// Processus et fragment de titre exigés.
    pub expected: WindowDescription,
    /// Fenêtre au premier plan, `null` si elle n'est pas lisible.
    pub foreground: Option<WindowDescription>,
}

/// Rapport de refus d'une frappe : `{typed:false, reason, expected, foreground}`, lu par `checkTyped` de Node.
#[derive(Serialize)]
struct TypedRefusal<'a> {
    typed: bool,
    #[serde(flatten)]
    refusal: &'a TargetRefusal,
}

/// Rapport de refus d'une mise au premier plan : `{focused:false, reason, expected, foreground}`.
#[derive(Serialize)]
struct FocusRefusal<'a> {
    focused: bool,
    #[serde(flatten)]
    refusal: &'a TargetRefusal,
}

/// Rapport d'une frappe réussie ; il ne porte jamais la valeur.
#[derive(Serialize)]
struct TypedReport<'a> {
    typed: bool,
    field: &'a str,
    #[serde(rename = "clipboardUsed")]
    clipboard_used: bool,
    target: &'a WindowDescription,
}

/// Rapport d'une mise au premier plan réussie.
#[derive(Serialize)]
struct FocusedReport<'a> {
    focused: bool,
    target: &'a WindowDescription,
}

/// Sérialise une valeur en une ligne JSON.
///
/// Paramètres : la valeur.
/// Retour : la ligne, ou `BadResponse` si la sérialisation échoue.
///
/// # Errors
///
/// `BadResponse` si la valeur n'est pas encodable.
fn line<T: Serialize>(value: &T) -> Result<String, ClientError> {
    serde_json::to_string(value).map_err(|_| ClientError::BadResponse("Output not encodable.".to_owned()))
}

/// Ligne JSON de `foreground`.
///
/// Paramètres : la fenêtre au premier plan.
/// Retour : `{"process":…,"title":…}`.
///
/// # Errors
///
/// `BadResponse` si la sérialisation échoue.
pub fn foreground_line(window: &WindowDescription) -> Result<String, ClientError> {
    line(window)
}

/// Ligne JSON d'une frappe réussie.
///
/// Paramètres : le champ tapé et la fenêtre cible.
/// Retour : `{"typed":true,"field":…,"clipboardUsed":false,"target":{…}}`.
///
/// # Errors
///
/// `BadResponse` si la sérialisation échoue.
pub fn typed_line(field: &str, target: &WindowDescription) -> Result<String, ClientError> {
    line(&TypedReport {
        typed: true,
        field,
        clipboard_used: false,
        target,
    })
}

/// Ligne JSON d'une mise au premier plan réussie.
///
/// Paramètres : la fenêtre mise au premier plan.
/// Retour : `{"focused":true,"target":{…}}`.
///
/// # Errors
///
/// `BadResponse` si la sérialisation échoue.
pub fn focused_line(target: &WindowDescription) -> Result<String, ClientError> {
    line(&FocusedReport { focused: true, target })
}

/// Ligne JSON du refus d'une frappe.
///
/// Paramètres : le refus.
/// Retour : `{"typed":false,"reason":…,"expected":{…},"foreground":{…}|null}`.
///
/// # Errors
///
/// `BadResponse` si la sérialisation échoue.
pub fn typed_refusal_line(refusal: &TargetRefusal) -> Result<String, ClientError> {
    line(&TypedRefusal { typed: false, refusal })
}

/// Ligne JSON du refus d'une mise au premier plan.
///
/// Paramètres : le refus.
/// Retour : `{"focused":false,"reason":…,"expected":{…},"foreground":{…}|null}`.
///
/// # Errors
///
/// `BadResponse` si la sérialisation échoue.
pub fn focus_refusal_line(refusal: &TargetRefusal) -> Result<String, ClientError> {
    line(&FocusRefusal {
        focused: false,
        refusal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// Fenêtre décrite de test.
    fn window(process: &str, title: &str) -> WindowDescription {
        WindowDescription {
            process: process.to_owned(),
            title: title.to_owned(),
        }
    }

    /// Refus de test avec la fenêtre active donnée.
    fn refusal(foreground: Option<WindowDescription>) -> TargetRefusal {
        TargetRefusal {
            reason: "The foreground window changed.".to_owned(),
            expected: window("chrome", "Ma banque"),
            foreground,
        }
    }

    /// Lit une ligne de sortie comme JSON.
    fn parse(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn typed_refusal_has_the_shape_node_reads() {
        let text = typed_refusal_line(&refusal(Some(window("notepad", "Sans titre")))).unwrap();
        assert_eq!(
            parse(&text),
            json!({
                "typed": false,
                "reason": "The foreground window changed.",
                "expected": {"process": "chrome", "title": "Ma banque"},
                "foreground": {"process": "notepad", "title": "Sans titre"}
            })
        );
        assert!(!text.contains('\n'));
    }

    #[test]
    fn unreadable_foreground_is_null() {
        let value = parse(&typed_refusal_line(&refusal(None)).unwrap());
        assert!(value["foreground"].is_null());
        assert_eq!(value["typed"], false);
    }

    #[test]
    fn focus_refusal_uses_the_focused_key() {
        let value = parse(&focus_refusal_line(&refusal(None)).unwrap());
        assert_eq!(value["focused"], false);
        assert!(value.get("typed").is_none());
        assert_eq!(value["expected"]["process"], "chrome");
    }

    #[test]
    fn success_reports_never_carry_a_value_or_a_version() {
        let target = window("chrome", "Ma banque");
        assert_eq!(
            parse(&typed_line("password", &target).unwrap()),
            json!({"typed": true, "field": "password", "clipboardUsed": false, "target": {"process": "chrome", "title": "Ma banque"}})
        );
        assert_eq!(
            parse(&focused_line(&target).unwrap()),
            json!({"focused": true, "target": {"process": "chrome", "title": "Ma banque"}})
        );
        assert_eq!(
            parse(&foreground_line(&target).unwrap()),
            json!({"process": "chrome", "title": "Ma banque"})
        );
    }
}
