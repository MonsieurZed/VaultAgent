//! Page principale en quatre blocs : barre, Coffre, « À valider », Appareils, Ajouter un appareil, mode « Modifier »,
//! aide, rafraîchissement, feuille de style et vocabulaire.

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
fn the_main_page_has_a_thin_bar_and_four_placed_blocks_without_hero_or_footer() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let body = main_page(&service);
    assert!(body.contains("<div class=\"blocks\">") && !body.contains("blocks edit"));
    let positions: Vec<usize> = [
        "class=\"bar\"",
        "class=\"card b-vault\"",
        "class=\"card b-pending\"",
        "class=\"card b-devices\"",
        "class=\"card b-add\"",
    ]
    .iter()
    .map(|marker| body.find(marker).unwrap())
    .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(body.contains(
        "probe@example.invalid sur vault.example.test · verrouillage après 15 min · <a href=\"/?view=vault\">Modifier</a>"
    ));
    assert!(body.contains("<a href=\"/?view=help\">Aide</a>") && body.contains("<h2>Ajouter un appareil</h2>"));
    assert!(!body.contains("view=add") && !body.contains("class=\"vaultline\""));
    for gone in ["class=\"intro\"", "eyebrow", "<footer", "Votre coffre, disponible"] {
        assert!(!body.contains(gone), "{gone}");
    }
}

#[test]
fn the_bar_carries_neither_the_state_nor_the_vault_buttons() {
    let service = Service::new(VaultState::Locked, Client::None);
    let body = main_page(&service);
    let bar = &body[body.find("<header").unwrap()..body.find("</header>").unwrap()];
    assert!(!bar.contains("Verrouillé") && !bar.contains("<button") && !bar.contains("<input"));
    assert!(bar.contains("Actualiser") && bar.contains("Aide") && bar.contains("hreflang=\"es\""));
}

#[test]
fn the_add_block_holds_the_form_and_the_downloads() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let body = main_page(&service);
    let block = &body[body.find("class=\"card b-add\"").unwrap()..];
    assert!(block.contains("action=\"/admin/pairing\"") && block.contains("name=\"name\""));
    assert!(block.contains("Créer un code d’ajout"));
    assert!(block.contains("windows-client.zip.sha256") && block.contains("linux-client.tar.gz.sha256"));
    assert!(block.find("action=\"/admin/pairing\"").unwrap() < block.find("windows-client.zip").unwrap());
}

#[test]
fn the_vault_summary_says_when_there_is_no_automatic_lock() {
    let service = Service::new(VaultState::Unconfigured, Client::None);
    connectable(&service.factory);
    let saved = "server=https%3A%2F%2Fvault.example.test%2F&email=probe%40example.invalid&password=x&code=";
    assert_eq!(service.send(&form("configure", saved)).status, 303);
    let body = main_page(&service);
    assert!(body.contains("sur vault.example.test · sans verrouillage automatique ·"));
}

#[test]
fn the_state_and_the_button_follow_the_state_with_their_colour_class() {
    let locked = main_page(&Service::new(VaultState::Locked, Client::None));
    assert!(locked.contains("<p class=\"state locked\"><strong>Verrouillé</strong></p>"));
    assert!(locked.contains("action=\"/admin/unlock\"") && locked.contains("name=\"code\""));
    assert!(!locked.contains("action=\"/admin/lock\"") && !locked.contains("state open"));
    let open = main_page(&Service::new(VaultState::Unlocked, Client::None));
    assert!(open.contains("<p class=\"state open\"><strong>Déverrouillé jusqu’à 09:15</strong></p>"));
    assert!(open.contains("action=\"/admin/lock\"") && !open.contains("action=\"/admin/unlock\""));
    assert!(!open.contains("state locked"));
}

#[test]
fn the_to_approve_block_is_always_there_with_an_empty_state_or_its_requests() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let empty = main_page(&service);
    assert!(empty.contains("<h2>À valider</h2>") && empty.contains("class=\"card b-pending\""));
    assert!(
        empty.contains("<div class=\"empty\"><strong>Rien à valider</strong><p>La page se met à jour seule.</p></div>")
    );
    assert!(!empty.contains("decide-unlock"));
    pending(&service);
    let body = main_page(&service);
    assert!(body.contains("<h2>À valider <span class=\"count\">2</span></h2>") && !body.contains("Rien à valider"));
    assert!(body.contains("action=\"/admin/decide-unlock\"") && body.contains("action=\"/admin/pair-decide\""));
    assert!(body.contains(">Déverrouiller</button>") && body.contains(">Ajouter</button>"));
    assert!(body.find("class=\"card b-pending\"").unwrap() < body.find("class=\"card b-devices\"").unwrap());
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
fn the_help_page_is_one_wide_block_with_a_back_link_and_no_refresh() {
    let service = Service::new(VaultState::Locked, Client::Paired);
    let help = html(&service.send(&get("/?view=help")));
    assert!(help.contains("<a class=\"back\" href=\"/\">Retour</a>") && !help.contains("http-equiv=\"refresh\""));
    assert!(help.contains("class=\"card single\"") && !help.contains("class=\"blocks"));
    assert!(help.contains("Version du serveur") && help.contains("3.0.0-preview.7"));
    assert!(help.contains("Ajouter un appareil"));
}

#[test]
fn the_edit_mode_puts_the_form_in_the_vault_block_and_keeps_the_other_blocks() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let body = html(&service.send(&get("/?view=vault")));
    assert!(body.contains("<div class=\"blocks edit\">") && !body.contains("http-equiv=\"refresh\""));
    let form = body.find("action=\"/admin/configure\"").unwrap();
    assert!(
        body.find("class=\"card b-vault\"").unwrap() < form && form < body.find("class=\"card b-pending\"").unwrap()
    );
    assert!(body.contains("Changer de compte retire tous les appareils."));
    assert!(body.contains("<a class=\"button quiet\" href=\"/\">Annuler</a>"));
    for block in ["b-pending", "b-devices", "b-add"] {
        assert!(body.contains(&format!("class=\"card {block}\"")), "{block}");
    }
    assert!(!body.contains("class=\"summary\"") && !body.contains("action=\"/admin/lock\""));
}

#[test]
fn an_unconfigured_vault_opens_the_edit_mode_without_a_cancel_link() {
    let service = Service::new(VaultState::Unconfigured, Client::None);
    let body = main_page(&service);
    assert!(body.contains("<div class=\"blocks edit\">") && body.contains("action=\"/admin/configure\""));
    assert!(body.contains("chip setup") && !body.contains("Annuler"));
    assert!(!body.contains("http-equiv=\"refresh\""));
}

#[test]
fn the_stylesheet_places_the_blocks_on_three_full_window_columns() {
    let css = include_str!("../assets/style.css");
    assert!(css.contains("grid-template-columns:repeat(3,minmax(0,1fr))"));
    assert!(css.contains("grid-template-rows:minmax(0,45fr) minmax(0,55fr)"));
    assert!(css.contains("grid-template-areas:\"vault pending pending\" \"devices devices add\""));
    assert!(css.contains(".blocks.edit{grid-template-areas:\"vault pending pending\" \"vault devices add\"}"));
    assert!(css.contains("@media(max-width:1000px)") && !css.contains("max-width:1100px"));
    for class in [
        ".b-vault",
        ".b-pending",
        ".b-devices",
        ".b-add",
        ".state.open",
        ".state.locked",
    ] {
        assert!(css.contains(class), "{class}");
    }
    assert!(css.contains("--ok:") && css.contains("--warn:") && css.contains("--danger:"));
}

#[test]
fn the_refresh_meta_exists_only_on_a_quiet_main_page() {
    let meta = "http-equiv=\"refresh\" content=\"10\"";
    let quiet = Service::new(VaultState::Locked, Client::None);
    assert!(main_page(&quiet).contains(meta));
    assert!(!html(&quiet.send(&get("/?msg=saved"))).contains(meta));
    assert!(!html(&quiet.send(&get("/?view=help"))).contains(meta));
    assert!(!html(&quiet.send(&get("/?view=vault"))).contains(meta));
    let busy = Service::new(VaultState::Locked, Client::Paired);
    pending(&busy);
    assert!(!main_page(&busy).contains("http-equiv=\"refresh\""));
    let unconfigured = Service::new(VaultState::Unconfigured, Client::None);
    assert!(!main_page(&unconfigured).contains("http-equiv=\"refresh\""));
}

#[test]
fn the_message_sits_in_the_bar_and_leaves_the_blocks_alone() {
    let service = Service::new(VaultState::Unlocked, Client::Paired);
    let body = html(&service.send(&get("/?msg=saved")));
    let bar = &body[body.find("<header").unwrap()..body.find("</header>").unwrap()];
    assert!(bar.contains("<p class=\"message\" role=\"status\">Enregistré.</p>"));
    assert!(body.contains("class=\"summary\"") && body.contains("class=\"card b-devices\""));
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
    for view in ["", "vault", "help"] {
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
