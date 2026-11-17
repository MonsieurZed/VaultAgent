//! Page du code d'ajout : servie directement par `POST /admin/pairing`, jamais par redirection, pour que le
//! code n'entre dans aucune URL.

use crate::front_dates::time_of_ms;
use crate::front_escape::escape;
use crate::front_lang::Lang;
use crate::front_response::page;
use crate::http::Response;
use crate::i18n::text;

/// Page qui montre le code dans un bloc repliable, avec son heure d'expiration et un lien de retour.
///
/// Paramètres : la langue, le code (`<coffre>.<code>`), l'instant d'expiration en millisecondes depuis l'époque.
/// Retour : la réponse `200` (`Cache-Control: no-store`).
#[must_use]
pub fn pairing_page(lang: Lang, code: &str, expires_ms: u64) -> Response {
    let body = format!(
        "<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
content=\"width=device-width,initial-scale=1\"><title>VaultAgent</title><link rel=\"stylesheet\" href=\"/style.css\"></head>\n\
<body><header class=\"bar\"><a class=\"brand\" href=\"/\"><span class=\"shield\">◆</span> VaultAgent</a></header>\n\
<main><section class=\"tab pairing\"><h2>{}</h2><p>{}</p><details><summary>{}</summary><code class=\"pair-code\">{}</code>\
</details><p class=\"note\">{} {}</p><p><a href=\"/\">{}</a></p></section></main></body></html>\n",
        lang.code(),
        text(lang, "Code d’ajout"),
        text(
            lang,
            "Saisissez ce code dans le programme de l’appareil. Usage unique, valable cinq minutes. Ne le collez pas dans un chat."
        ),
        text(lang, "Afficher le code"),
        escape(code),
        text(lang, "Valable jusqu’à"),
        escape(&time_of_ms(expires_ms)),
        text(lang, "Retour"),
    );
    page(body, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_is_inside_a_details_block_and_the_page_is_not_cached() {
        let response = pairing_page(Lang::En, "default.abc", 1_800_000_000_000);
        let html = String::from_utf8(response.body.clone()).unwrap();
        assert!(html.contains("<details><summary>Show code</summary><code class=\"pair-code\">default.abc</code>"));
        assert!(html.contains("09:00") && !html.contains("<script"));
        assert_eq!(response.header("cache-control"), Some("no-store"));
    }
}
