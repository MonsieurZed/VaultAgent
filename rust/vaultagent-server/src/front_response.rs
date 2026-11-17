//! Réponses du front : page HTML, redirection 303 vers la page et feuille de style, avec les en-têtes de sécurité.

use crate::front_lang::Lang;
use crate::front_notice::Notice;
use crate::front_screen::Screen;
use crate::http::Response;

/// Politique de sécurité du contenu : aucun script, formulaires vers l'origine seulement.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; style-src 'self'; img-src 'self'; form-action 'self'; \
frame-ancestors 'none'; base-uri 'none'";

const STYLE: &str = include_str!("../assets/style.css");

/// Politique de référent : rien hors de l'origine ; `no-referrer` ferait poster les formulaires avec `Origin: null`,
/// que le contrôle d'origine des actions refuse.
pub const REFERRER_POLICY: &str = "same-origin";

/// En-têtes de sécurité communs aux fichiers du front.
fn secured(kind: &str) -> Vec<(&'static str, String)> {
    vec![
        ("content-type", kind.to_owned()),
        ("cache-control", "no-store".to_owned()),
        ("content-security-policy", CONTENT_SECURITY_POLICY.to_owned()),
        ("x-content-type-options", "nosniff".to_owned()),
        ("referrer-policy", REFERRER_POLICY.to_owned()),
    ]
}

/// Page HTML en `200`.
///
/// Paramètres : le corps, la langue à mémoriser dans le cookie si elle vient d'être choisie.
/// Retour : la réponse.
#[must_use]
pub fn page(body: String, remember: Option<Lang>) -> Response {
    let mut headers = secured("text/html; charset=utf-8");
    if let Some(lang) = remember {
        headers.push(("set-cookie", lang.set_cookie()));
    }
    Response {
        status: 200,
        headers,
        body: body.into_bytes(),
    }
}

/// Feuille de style du front.
///
/// Paramètres : aucun.
/// Retour : la réponse `200` en `text/css`.
#[must_use]
pub fn style() -> Response {
    Response {
        status: 200,
        headers: secured("text/css; charset=utf-8"),
        body: STYLE.as_bytes().to_vec(),
    }
}

/// Redirection `303` vers la page, vers l'écran, avec le code du message.
///
/// Paramètres : l'écran, le message.
/// Retour : la réponse, sans corps.
#[must_use]
pub fn redirect(screen: Screen, notice: Notice) -> Response {
    Response {
        status: 303,
        headers: vec![
            ("location", screen.href(&[("msg", notice.code())])),
            ("cache-control", "no-store".to_owned()),
            ("x-content-type-options", "nosniff".to_owned()),
        ],
        body: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_forbids_scripts() {
        assert!(CONTENT_SECURITY_POLICY.starts_with("default-src 'none'"));
        assert!(!CONTENT_SECURITY_POLICY.contains("script-src"));
    }

    #[test]
    fn the_page_keeps_the_form_origin_and_hides_it_elsewhere() {
        let response = page(String::new(), None);
        assert_eq!(response.header("referrer-policy"), Some("same-origin"));
    }

    #[test]
    fn a_redirect_carries_only_codes() {
        let response = redirect(Screen::Vault, Notice::Expired);
        assert_eq!(
            (response.status, response.header("location")),
            (303, Some("/?view=vault&msg=expired"))
        );
    }

    #[test]
    fn a_remembered_language_sets_the_cookie() {
        let response = page(String::new(), Some(Lang::Es));
        assert!(
            response
                .header("set-cookie")
                .is_some_and(|value| value.contains("=es;"))
        );
        assert!(page(String::new(), None).header("set-cookie").is_none());
    }
}
