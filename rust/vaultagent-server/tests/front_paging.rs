//! Pagination des appareils (6 par page, dans le bloc), confirmation du retrait, heure de Paris et feuille de style.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::WALL_BASE;
use common::front::{form, get, html};
use common::service::{Client, Service, VaultState};
use serde_json::json;
use vaultagent_core::SeededEntropy;
use vaultagent_core::devices::enroll;
use vaultagent_server::Request;

/// Service verrouillé avec `count` appareils nommés « Poste 01 », « Poste 02 »… ; rend aussi leurs identifiants.
fn with_devices(count: usize) -> (Service, Vec<String>) {
    let service = Service::new(VaultState::Locked, Client::None);
    let vault = service.vaults.get(None).unwrap();
    let profile = vault.store.account().unwrap().profile;
    let mut entropy = SeededEntropy::new(9);
    let mut records = Vec::new();
    for index in 1..=count {
        let name = format!("Poste {index:02}");
        records = enroll(&name, &profile, None, WALL_BASE, &records, &mut entropy)
            .unwrap()
            .0;
    }
    let ids = records.iter().map(|record| record.id.clone()).collect();
    vault.store.update_devices(|_| Ok(records)).unwrap();
    (service, ids)
}

/// Ouvre `count` demandes d'ajout d'appareil.
fn pair_requests(service: &Service, count: usize) {
    for index in 0..count {
        let request = Request::new("POST", "/api/pair", "127.0.0.1")
            .with_header("content-type", "application/json")
            .with_body(
                json!({"action": "request", "name": format!("Demande {index}")})
                    .to_string()
                    .as_bytes(),
            );
        assert_eq!(service.send(&request).status, 200);
    }
}

/// Page rendue pour un chemin.
fn page(service: &Service, path: &str) -> String {
    html(&service.send(&get(path)))
}

/// Nombre d'appareils affichés.
fn shown(body: &str) -> usize {
    body.matches("Poste ").count()
}

#[test]
fn six_devices_fit_on_one_page_without_pager() {
    let (service, _) = with_devices(6);
    let body = page(&service, "/");
    assert_eq!(shown(&body), 6);
    assert!(!body.contains("class=\"pager\"") && body.contains("<span class=\"count\">6</span>"));
}

#[test]
fn seven_devices_are_paginated_with_links_both_ways() {
    let (service, _) = with_devices(7);
    let first = page(&service, "/");
    assert_eq!(shown(&first), 6);
    assert!(first.contains("<span>1 / 2</span>") && first.contains("href=\"/?page=2\">Suivant ›"));
    assert!(!first.contains("Précédent"));
    let second = page(&service, "/?page=2");
    assert_eq!(shown(&second), 1);
    assert!(second.contains("Poste 07") && second.contains("<span>2 / 2</span>"));
    assert!(second.contains("href=\"/\">‹ Précédent") && !second.contains("Suivant ›"));
}

#[test]
fn an_invalid_or_out_of_range_page_is_clamped() {
    let (service, _) = with_devices(7);
    for (query, label) in [
        ("0", "1 / 2"),
        ("abc", "1 / 2"),
        ("-3", "1 / 2"),
        ("99", "2 / 2"),
        ("", "1 / 2"),
    ] {
        let response = service.send(&get(&format!("/?page={query}")));
        assert_eq!(response.status, 200, "{query}");
        assert!(html(&response).contains(&format!("<span>{label}</span>")), "{query}");
    }
}

#[test]
fn waiting_requests_no_longer_shrink_the_device_page() {
    let (service, _) = with_devices(10);
    pair_requests(&service, 2);
    let body = page(&service, "/");
    assert_eq!(body.matches("Poste ").count(), 6);
    assert!(body.contains("<span>1 / 2</span>"));
    assert!(body.contains("<h2>À valider <span class=\"count\">2</span></h2>"));
}

#[test]
fn removing_is_a_link_until_confirmed() {
    let (service, ids) = with_devices(2);
    let body = page(&service, "/");
    assert!(body.contains(&format!("href=\"/?confirm={}\">Retirer", ids[0])));
    assert!(!body.contains("action=\"/admin/revoke\"") && !body.contains("Il ne pourra plus"));
    let confirm = page(&service, &format!("/?confirm={}", ids[0]));
    assert!(confirm.contains("<small class=\"danger\">Il ne pourra plus rien remplir.</small>"));
    assert!(confirm.contains("action=\"/admin/revoke\"") && confirm.contains(&format!("value=\"{}\"", ids[0])));
    assert!(confirm.contains("Confirmer le retrait") && confirm.contains("href=\"/\">Annuler"));
    assert_eq!(confirm.matches("Confirmer le retrait").count(), 1);
    assert!(confirm.contains(&format!("href=\"/?confirm={}\">Retirer", ids[1])));
}

#[test]
fn an_unknown_confirm_is_ignored_and_escaped_values_never_leak() {
    let (service, _) = with_devices(2);
    let body = page(&service, "/?confirm=%22%3E%3Cscript%3Ealert(1)%3C/script%3E");
    assert!(!body.contains("Confirmer le retrait") && !body.contains("<script"));
    assert!(body.contains("http-equiv=\"refresh\""));
}

#[test]
fn the_confirmation_stops_the_refresh_and_the_revoke_post_keeps_the_origin_check() {
    let (service, ids) = with_devices(1);
    assert!(!page(&service, &format!("/?confirm={}", ids[0])).contains("http-equiv=\"refresh\""));
    let body = format!("deviceId={}", ids[0]);
    let refused = common::front::form_as("revoke", &body, Some("https://autre.example"), None, "form");
    assert_ne!(service.send(&refused).status, 200);
    assert!(page(&service, "/").contains("Poste 01"));
    assert_eq!(service.send(&form("revoke", &body)).status, 303);
    assert!(page(&service, "/").contains("Aucun appareil."));
}

#[test]
fn dates_are_shown_in_paris_time_without_a_zone() {
    let (service, _) = with_devices(1);
    let body = page(&service, "/");
    assert!(body.contains("ajouté le 15/01") && !body.contains("UTC"));
    let open = page(&Service::new(VaultState::Unlocked, Client::None), "/");
    assert!(open.contains("Déverrouillé jusqu’à 09:15") && !open.contains("UTC"));
}

#[test]
fn the_stylesheet_fills_the_window_and_supports_the_dark_theme() {
    let css = include_str!("../assets/style.css");
    assert!(css.contains("height:100vh") && css.contains("grid-template-rows:40px"));
    assert!(css.contains("@media(prefers-color-scheme:dark)"));
    assert!(css.contains("height:40px") && css.contains("font:14px/1.3"));
    assert!(css.len() < 8_000);
}
