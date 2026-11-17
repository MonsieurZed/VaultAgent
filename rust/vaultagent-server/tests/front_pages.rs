//! Rendu de `GET /` et des pages secondaires : trois langues, échappement, champs secrets vides, absence de script,
//! cookie de langue, anciens liens `?tab=` ignorés.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::front::{get, get_with_cookie, html};
use common::service::{Client, Service, VaultState};
use common::{EMAIL, FICTITIOUS, SERVER, WALL_BASE};
use serde_json::json;
use vaultagent_core::SeededEntropy;
use vaultagent_core::devices::enroll;
use vaultagent_server::Request;

const VIEWS: [&str; 3] = ["", "vault", "help"];
const HOSTILE: &str = "<script>alert(1)</script>";

/// Page rendue pour un écran (`""` : la page principale) et une langue.
fn page(service: &Service, view: &str, lang: &str) -> String {
    let path = if view.is_empty() {
        format!("/?lang={lang}")
    } else {
        format!("/?view={view}&lang={lang}")
    };
    html(&service.send(&get(&path)))
}

/// Ouvre une demande de déverrouillage et une demande d'ajout d'appareil.
fn pending(service: &Service, name: &str) {
    service.call("request-unlock", &json!({}));
    let request = Request::new("POST", "/api/pair", "127.0.0.1")
        .with_header("content-type", "application/json")
        .with_body(json!({"action": "request", "name": name}).to_string().as_bytes());
    assert_eq!(service.send(&request).status, 200);
}

#[test]
fn each_page_shows_its_key_texts_in_the_three_languages() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let expected = [
        ["Appareils", "Devices", "Dispositivos"],
        ["Connecter votre coffre", "Connect your vault", "Conectar tu bóveda"],
        [
            "Ce que protège le remplissage",
            "What blind filling protects",
            "Qué protege el rellenado",
        ],
    ];
    for (view, texts) in VIEWS.iter().zip(expected) {
        for (lang, text) in ["fr", "en", "es"].iter().zip(texts) {
            let body = page(&service, view, lang);
            assert!(body.contains(text), "{view}/{lang}");
            assert!(body.contains(&format!("<html lang=\"{lang}\">")));
        }
    }
    let status = [("fr", "Verrouillé"), ("en", "Locked"), ("es", "Bloqueada")];
    for (lang, text) in status {
        assert!(page(&service, "", lang).contains(text));
    }
    let blocks = [("fr", "Coffre"), ("en", "Vault"), ("es", "Bóveda")];
    for (lang, text) in blocks {
        assert!(page(&service, "", lang).contains(&format!("<h2>{text}</h2>")));
    }
    let empty = [
        ("fr", "Rien à valider"),
        ("en", "Nothing to approve"),
        ("es", "Nada que aprobar"),
    ];
    for (lang, text) in empty {
        assert!(page(&Service::new(VaultState::Locked, Client::None), "", lang).contains(text));
    }
}

#[test]
fn only_the_requested_page_is_rendered_and_an_old_tab_link_is_ignored() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let main = page(&service, "", "fr");
    assert!(main.contains("Client factice") && !main.contains("Connecter votre coffre"));
    let vault = page(&service, "vault", "fr");
    assert!(vault.contains("Connecter votre coffre") && vault.contains("Client factice"));
    assert!(!vault.contains("Verrouillé</strong>") && !vault.contains("action=\"/admin/unlock\""));
    assert!(!vault.contains("Ce que protège le remplissage"));
    assert!(main.contains("/downloads/linux-client.tar.gz"));
    let removed = page(&service, "add", "fr");
    assert!(removed.contains("Client factice") && !removed.contains("Connecter votre coffre"));
    for old in ["account", "clients", "about", "nope"] {
        let legacy = html(&service.send(&get(&format!("/?tab={old}&lang=xx"))));
        assert!(
            legacy.contains("Client factice") && legacy.contains("<html lang=\"fr\">"),
            "{old}"
        );
    }
    let unknown_view = html(&service.send(&get("/?view=settings")));
    assert!(unknown_view.contains("Client factice"));
}

#[test]
fn the_language_link_sets_a_strict_cookie_and_the_cookie_is_reused() {
    let service = Service::new(VaultState::Locked, Client::None);
    let chosen = service.send(&get("/?lang=es"));
    let cookie = chosen.header("set-cookie").unwrap();
    for part in [
        "vaultagent-language=es",
        "Path=/",
        "SameSite=Strict",
        "HttpOnly",
        "Secure",
    ] {
        assert!(cookie.contains(part), "{part}");
    }
    let later = html(&service.send(&get_with_cookie("/", "vaultagent-language=es")));
    assert!(later.contains("Dispositivos"));
    let ignored = service.send(&get("/?lang=zz"));
    assert!(ignored.header("set-cookie").is_none() && html(&ignored).contains("Appareils"));
    let kept = page(&service, "help", "en");
    assert!(kept.contains("href=\"/?view=help&amp;lang=es\""));
}

#[test]
fn hostile_names_are_escaped() {
    let service = Service::new(VaultState::Locked, Client::None);
    let vault = service.vaults.get(None).unwrap();
    let profile = vault.store.account().unwrap().profile;
    let (records, _) = enroll(HOSTILE, &profile, None, WALL_BASE, &[], &mut SeededEntropy::new(3)).unwrap();
    vault.store.update_devices(|_| Ok(records)).unwrap();
    pending(&service, HOSTILE);
    let body = page(&service, "", "en");
    assert!(!body.contains("<script"), "{body}");
    assert!(body.matches("&lt;script&gt;alert(1)&lt;/script&gt;").count() >= 2);
}

#[test]
fn password_and_code_fields_are_always_rendered_empty() {
    let service = Service::new(VaultState::Unlocked, Client::None);
    let body = page(&service, "vault", "fr");
    assert!(body.contains("name=\"password\" type=\"password\" autocomplete=\"new-password\" value=\"\""));
    assert!(body.contains("name=\"code\" autocomplete=\"one-time-code\" value=\"\""));
    assert!(body.contains(&format!("value=\"{SERVER}\"")) && body.contains(&format!("value=\"{EMAIL}\"")));
    assert!(!body.contains(FICTITIOUS));
    assert!(body.contains("Changer de compte retire tous les appareils."));
}

#[test]
fn no_page_carries_a_script_or_an_event_handler() {
    for (state, client) in [
        (VaultState::Unconfigured, Client::None),
        (VaultState::Unlocked, Client::Paired),
    ] {
        let service = Service::new(state, client);
        pending(&service, "Poste");
        for view in VIEWS {
            for lang in ["fr", "en", "es"] {
                let body = page(&service, view, lang).to_lowercase();
                for forbidden in [
                    "<script",
                    "javascript:",
                    " onclick=",
                    " onchange=",
                    " onsubmit=",
                    " onload=",
                ] {
                    assert!(!body.contains(forbidden), "{view}/{lang}/{forbidden}");
                }
            }
        }
    }
}

#[test]
fn times_are_server_rendered_in_paris_time_and_messages_are_translated_codes() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let body = page(&service, "", "fr");
    assert!(body.contains("Déverrouillé jusqu’à 09:15"));
    assert!(body.contains("ajouté le 15/01"), "date des appareils");
    let refused = html(&service.send(&get("/?view=vault&msg=login-refused&lang=en")));
    assert!(refused.contains("Sign-in refused."));
    let injected = html(&service.send(&get("/?msg=%3Cscript%3E")));
    assert!(!injected.contains("<script") && !injected.contains("&lt;script"));
}

#[test]
fn an_unconfigured_vault_shows_the_vault_form_on_the_root_page() {
    let service = Service::new(VaultState::Unconfigured, Client::None);
    let body = page(&service, "", "en");
    assert!(body.contains("Setup required") && body.contains("class=\"blocks edit\""));
    assert!(body.contains("Connect your vault") && body.contains("action=\"/admin/configure\""));
    assert!(!body.contains("action=\"/admin/unlock\"") && !body.contains("class=\"back\""));
}
