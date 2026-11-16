//! Rendu de `GET /` : onglets, trois langues, échappement, champs secrets vides, absence de script, rafraîchissement.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::front::{get, get_with_cookie, html};
use common::service::{Client, Service, VaultState};
use common::{EMAIL, FICTITIOUS, SERVER, WALL_BASE};
use serde_json::json;
use vaultagent_core::SeededEntropy;
use vaultagent_core::devices::enroll;
use vaultagent_server::Request;

const TABS: [&str; 3] = ["account", "clients", "about"];
const HOSTILE: &str = "<script>alert(1)</script>";

/// Page rendue pour un onglet et une langue.
fn page(service: &Service, tab: &str, lang: &str) -> String {
    html(&service.send(&get(&format!("/?tab={tab}&lang={lang}"))))
}

/// Ouvre une demande de déverrouillage et une demande d'association.
fn pending(service: &Service, name: &str) {
    service.call("request-unlock", &json!({}));
    let request = Request::new("POST", "/api/pair", "127.0.0.1")
        .with_header("content-type", "application/json")
        .with_body(json!({"action": "request", "name": name}).to_string().as_bytes());
    assert_eq!(service.send(&request).status, 200);
}

#[test]
fn each_tab_shows_its_key_texts_in_the_three_languages() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let expected = [
        ["Connecter votre coffre", "Connect your vault", "Conectar tu bóveda"],
        [
            "Vos clients Windows et Linux",
            "Your Windows and Linux clients",
            "Tus clientes Windows y Linux",
        ],
        [
            "Ce que protège le remplissage",
            "What blind filling protects",
            "Qué protege el rellenado",
        ],
    ];
    for (tab, texts) in TABS.iter().zip(expected) {
        for (lang, text) in ["fr", "en", "es"].iter().zip(texts) {
            let body = page(&service, tab, lang);
            assert!(body.contains(text), "{tab}/{lang}");
            assert!(body.contains(&format!("<html lang=\"{lang}\">")));
        }
    }
    let status = [
        ("fr", "Coffre verrouillé"),
        ("en", "Vault locked"),
        ("es", "Bóveda bloqueada"),
    ];
    for (lang, text) in status {
        assert!(page(&service, "account", lang).contains(text));
    }
}

#[test]
fn only_the_active_tab_is_rendered() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let account = page(&service, "account", "fr");
    assert!(account.contains("Connecter votre coffre") && !account.contains("Vos clients Windows et Linux"));
    assert!(!account.contains("Ce que protège le remplissage"));
    let clients = page(&service, "clients", "fr");
    assert!(clients.contains("/downloads/linux-client.tar.gz") && !clients.contains("Connecter votre coffre"));
    assert!(clients.contains("Client factice"));
    let unknown = html(&service.send(&get("/?tab=nope&lang=xx")));
    assert!(unknown.contains("Connecter votre coffre") && unknown.contains("<html lang=\"fr\">"));
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
    assert!(later.contains("Conectar tu bóveda"));
    let ignored = service.send(&get("/?lang=zz"));
    assert!(ignored.header("set-cookie").is_none() && html(&ignored).contains("Connecter votre coffre"));
}

#[test]
fn hostile_names_are_escaped() {
    let service = Service::new(VaultState::Locked, Client::None);
    let vault = service.vaults.get(None).unwrap();
    service.vaults.save_name(Some("default"), Some(HOSTILE)).unwrap();
    let profile = vault.store.account().unwrap().profile;
    let (records, _) = enroll(HOSTILE, &profile, None, WALL_BASE, &[], &mut SeededEntropy::new(3)).unwrap();
    vault.store.update_devices(|_| Ok(records)).unwrap();
    pending(&service, HOSTILE);
    let body = page(&service, "clients", "en");
    assert!(!body.contains("<script"), "{body}");
    assert!(body.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(body.contains("Allow on &lt;script&gt;"));
}

#[test]
fn password_and_code_fields_are_always_rendered_empty() {
    let service = Service::new(VaultState::Unlocked, Client::None);
    let body = page(&service, "account", "fr");
    assert!(body.contains("name=\"password\" type=\"password\" autocomplete=\"new-password\" value=\"\""));
    assert!(body.contains("name=\"code\" autocomplete=\"one-time-code\" value=\"\""));
    assert!(body.contains(&format!("value=\"{SERVER}\"")) && body.contains(&format!("value=\"{EMAIL}\"")));
    assert!(!body.contains(FICTITIOUS));
}

#[test]
fn no_page_carries_a_script_or_an_event_handler() {
    for (state, client) in [
        (VaultState::Unconfigured, Client::None),
        (VaultState::Unlocked, Client::Paired),
    ] {
        let service = Service::new(state, client);
        pending(&service, "Poste");
        for tab in TABS {
            for lang in ["fr", "en", "es"] {
                let body = page(&service, tab, lang).to_lowercase();
                for forbidden in [
                    "<script",
                    "javascript:",
                    " onclick=",
                    " onchange=",
                    " onsubmit=",
                    " onload=",
                ] {
                    assert!(!body.contains(forbidden), "{tab}/{lang}/{forbidden}");
                }
            }
        }
    }
}

#[test]
fn the_refresh_meta_follows_the_tab_and_the_pending_requests() {
    let quiet = Service::new(VaultState::Locked, Client::None);
    let meta = "http-equiv=\"refresh\" content=\"5\"";
    assert!(page(&quiet, "clients", "fr").contains(meta));
    assert!(!page(&quiet, "account", "fr").contains(meta));
    assert!(!page(&quiet, "about", "fr").contains(meta));
    let after_action = html(&quiet.send(&get("/?tab=clients&msg=saved")));
    assert!(!after_action.contains(meta));
    let busy = Service::new(VaultState::Locked, Client::Paired);
    pending(&busy, "Poste");
    assert!(page(&busy, "about", "fr").contains(meta));
    assert!(html(&busy.send(&get("/?tab=clients&msg=saved"))).contains(meta));
    assert!(!page(&busy, "account", "fr").contains(meta));
    assert!(page(&busy, "account", "fr").contains("Demandes de déverrouillage"));
    assert!(page(&busy, "clients", "fr").contains("Demandes d’association"));
}

#[test]
fn times_are_server_rendered_in_utc_and_messages_are_translated_codes() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let body = page(&service, "clients", "fr");
    assert!(body.contains("08:15 UTC"));
    assert!(body.contains("2027-01-15"), "date des appareils");
    let refused = html(&service.send(&get("/?tab=account&msg=login-refused&lang=en")));
    assert!(refused.contains("Sign-in refused."));
    let injected = html(&service.send(&get("/?msg=%3Cscript%3E")));
    assert!(!injected.contains("<script") && !injected.contains("&lt;script"));
}

#[test]
fn an_unconfigured_vault_asks_for_setup_and_disables_the_actions() {
    let service = Service::new(VaultState::Unconfigured, Client::None);
    let body = page(&service, "account", "en");
    assert!(body.contains("Setup required") && body.contains("disabled"));
}
