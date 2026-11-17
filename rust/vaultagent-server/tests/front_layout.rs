//! Page principale unique : barre, ligne du coffre, section « À valider », appareils, pages secondaires, rafraîchissement
//! et vocabulaire.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::front::{form, get, html};
use common::service::{Client, Service, VaultState};
use common::{WALL_BASE, connectable};
use serde_json::json;
use vaultagent_core::SeededEntropy;
use vaultagent_core::devices::enroll;
use vaultagent_server::Request;

/// Page principale en français.
fn main_page(service: &Service) -> String {
    html(&service.send(&get("/")))
}

/// Ouvre une demande de déverrouillage et une demande d'ajout d'appareil.
fn pending(service: &Service) {
    service.call("request-unlock", &json!({}));
    let request = Request::new("POST", "/api/pair", "127.0.0.1")
        .with_header("content-type", "application/json")
        .with_body(json!({"action": "request", "name": "Poste"}).to_string().as_bytes());
    assert_eq!(service.send(&request).status, 200);
}

#[test]
fn the_main_page_stacks_bar_vault_line_and_devices_without_hero_or_footer() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let body = main_page(&service);
    let positions: Vec<usize> = ["class=\"bar\"", "class=\"vaultline\"", "class=\"devices\""]
        .iter()
        .map(|marker| body.find(marker).unwrap())
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(body.contains(
        "probe@example.invalid sur vault.example.test · verrouillage après 15 min · <a href=\"/?view=vault\">Modifier</a>"
    ));
    assert!(
        body.contains("<a href=\"/?view=help\">Aide</a>") && body.contains("href=\"/?view=add\">Ajouter un appareil")
    );
    for gone in ["class=\"intro\"", "eyebrow", "<footer", "Votre coffre, disponible"] {
        assert!(!body.contains(gone), "{gone}");
    }
}

#[test]
fn the_vault_line_says_when_there_is_no_automatic_lock() {
    let service = Service::new(VaultState::Unconfigured, Client::None);
    connectable(&service.factory);
    let saved = "server=https%3A%2F%2Fvault.example.test%2F&email=probe%40example.invalid&password=x&code=";
    assert_eq!(service.send(&form("configure", saved)).status, 303);
    let body = main_page(&service);
    assert!(body.contains("sur vault.example.test · sans verrouillage automatique ·"));
}

#[test]
fn the_pill_and_the_button_follow_the_state() {
    let locked = main_page(&Service::new(VaultState::Locked, Client::None));
    assert!(locked.contains("Verrouillé</strong>") && locked.contains("action=\"/admin/unlock\""));
    assert!(locked.contains("name=\"code\"") && !locked.contains("action=\"/admin/lock\""));
    let open = main_page(&Service::new(VaultState::Unlocked, Client::None));
    assert!(open.contains("Déverrouillé jusqu’à 09:15"));
    assert!(open.contains("action=\"/admin/lock\"") && !open.contains("action=\"/admin/unlock\""));
}

#[test]
fn the_to_approve_section_exists_only_when_a_request_waits() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    assert!(!main_page(&service).contains("À valider"));
    pending(&service);
    let body = main_page(&service);
    assert!(body.contains("<h2>À valider</h2>"));
    assert!(body.contains("action=\"/admin/decide-unlock\"") && body.contains("action=\"/admin/pair-decide\""));
    assert!(body.contains(">Déverrouiller</button>") && body.contains(">Ajouter</button>"));
    assert!(body.find("À valider").unwrap() < body.find("class=\"devices\"").unwrap());
}

#[test]
fn roles_and_modes_follow_what_the_server_knows() {
    let agent = main_page(&Service::new(VaultState::Unlocked, Client::Paired));
    assert!(agent.contains("<small>agent</small>") && agent.contains("Sans demander"));
    assert!(agent.contains("action=\"/admin/auto-unlock\"") && agent.contains("href=\"/?confirm="));
    assert!(!agent.contains("action=\"/admin/revoke\""));
    let card = main_page(&Service::new(VaultState::Unlocked, Client::Approver));
    assert!(card.contains("validateur · carte") && !card.contains("Sans demander") && !card.contains("auto-unlock"));
    assert!(card.contains("Retirer"));
    let uac = main_page(&Service::new(VaultState::Unlocked, Client::Elevated));
    assert!(uac.contains("validateur · UAC") && !uac.contains("Sans demander"));
    let hello = Service::new(VaultState::Unlocked, Client::Approver);
    let vault = hello.vaults.get(None).unwrap();
    vault
        .store
        .update_devices(|records| {
            let mut next = records.to_vec();
            next[0].hello_credential = Some(vaultagent_core::HelloCredential {
                id: "i".to_owned(),
                key_type: "EC2".to_owned(),
                first: "a".to_owned(),
                second: "b".to_owned(),
            });
            Ok(next)
        })
        .unwrap();
    assert!(main_page(&hello).contains("validateur · Hello"));
}

#[test]
fn every_secondary_page_has_a_back_link_and_no_refresh() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    for view in ["vault", "add", "help"] {
        let body = html(&service.send(&get(&format!("/?view={view}"))));
        assert!(body.contains("<a class=\"back\" href=\"/\">Retour</a>"), "{view}");
        assert!(!body.contains("http-equiv=\"refresh\""), "{view}");
        assert!(
            !body.contains("class=\"devices\"") && !body.contains("class=\"vaultline\""),
            "{view}"
        );
    }
    let help = html(&service.send(&get("/?view=help")));
    assert!(help.contains("Version du serveur") && help.contains("3.0.0-preview.7"));
    assert!(help.contains("Ajouter un appareil"));
    let add = html(&service.send(&get("/?view=add")));
    assert!(add.contains("windows-client.zip.sha256") && add.contains("linux-client.tar.gz.sha256"));
    assert!(add.contains("action=\"/admin/pairing\"") && add.contains("name=\"name\""));
    let vault = html(&service.send(&get("/?view=vault")));
    assert!(vault.contains("Changer de compte retire tous les appareils."));
}

#[test]
fn the_refresh_meta_exists_only_on_a_quiet_main_page() {
    let meta = "http-equiv=\"refresh\" content=\"10\"";
    let quiet = Service::new(VaultState::Locked, Client::None);
    assert!(main_page(&quiet).contains(meta));
    assert!(!html(&quiet.send(&get("/?msg=saved"))).contains(meta));
    assert!(!html(&quiet.send(&get("/?view=help"))).contains(meta));
    let busy = Service::new(VaultState::Locked, Client::Paired);
    pending(&busy);
    assert!(!main_page(&busy).contains("http-equiv=\"refresh\""));
    let unconfigured = Service::new(VaultState::Unconfigured, Client::None);
    assert!(!main_page(&unconfigured).contains("http-equiv=\"refresh\""));
}

#[test]
fn the_message_replaces_the_vault_line() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let body = html(&service.send(&get("/?msg=saved")));
    assert!(body.contains("<p class=\"message\" role=\"status\">Enregistré.</p>"));
    assert!(!body.contains("class=\"vaultline\""));
}

#[test]
fn the_french_pages_no_longer_use_the_old_vocabulary() {
    let service = Service::new(VaultState::Locked, Client::None);
    let vault = service.vaults.get(None).unwrap();
    let profile = vault.store.account().unwrap().profile;
    let mut entropy = SeededEntropy::new(5);
    let (records, _) = enroll("Poste A", &profile, None, WALL_BASE, &[], &mut entropy).unwrap();
    let (records, _) = enroll(
        "Poste B",
        &profile,
        Some("cle-factice"),
        WALL_BASE,
        &records,
        &mut entropy,
    )
    .unwrap();
    vault.store.update_devices(|_| Ok(records)).unwrap();
    pending(&service);
    for view in ["", "vault", "add", "help"] {
        let path = format!("/?view={view}");
        let body = html(&service.send(&get(&path))).to_lowercase();
        for old in [
            "client",
            "révoquer",
            "autoriser les clients",
            "association",
            "approbateur",
            "enveloppe",
            "toujours accepter",
        ] {
            let allowed = body
                .replace("windows-client.zip", "")
                .replace("linux-client.tar.gz", "");
            assert!(!allowed.contains(old), "{view}: {old}");
        }
    }
}
