//! Réponses du faux moteur pour chaque cas doré, dans l'ordre des appels du serveur Node.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vaultagent_engine::{Folder, Reply};

use crate::common::service::{FOLDER_ID, NEXT_REVISION, REVISION, demo_item, item_reply};

const NEW_ID: &str = "00000000-0000-4000-8000-0000000000d1";

/// Fiche créée par le corps des cas `write-create*`.
fn created_item() -> vaultagent_engine::Item {
    let mut item = demo_item(REVISION, "Nouvelle fiche factice");
    NEW_ID.clone_into(&mut item.id);
    if let Some(login) = item.login.as_mut() {
        login.username = Some("nouveau@example.invalid".to_owned());
        login.password = Some("Fictif-nouveau-42!".into());
        "https://example.test".clone_into(&mut login.uris[0].uri);
    }
    item
}

/// Réponses scriptées d'un cas, vides si le serveur ne doit pas appeler le moteur.
pub fn replies(case: &str) -> Vec<Reply> {
    let demo = || demo_item(REVISION, "Compte factice");
    match case {
        "catalog-unlocked" => vec![Reply::Unit, Reply::Items(vec![demo()])],
        "write-preview-list" => vec![
            Reply::Unit,
            Reply::Folders(vec![Folder {
                id: FOLDER_ID.to_owned(),
                name: "Dossier factice".to_owned(),
            }]),
            Reply::Items(vec![demo()]),
            Reply::Items(Vec::new()),
        ],
        "write-preview-detail" | "seal-envelope" | "seal-origin-refused" | "write-revision-conflict" => {
            vec![Reply::Unit, item_reply(demo())]
        }
        "write-create" | "write-create-replayed" | "write-operation-conflict" => vec![
            Reply::Unit,
            Reply::Items(Vec::new()),
            Reply::Text(NEW_ID.to_owned()),
            item_reply(created_item()),
        ],
        "write-create-folder" => vec![
            Reply::Unit,
            Reply::Folders(Vec::new()),
            Reply::Text(NEW_ID.to_owned()),
            Reply::Folders(vec![Folder {
                id: NEW_ID.to_owned(),
                name: "Nouveau dossier factice".to_owned(),
            }]),
        ],
        "write-update" => vec![
            Reply::Unit,
            item_reply(demo()),
            Reply::Unit,
            item_reply(demo_item(NEXT_REVISION, "Renommée factice")),
        ],
        "write-delete" => {
            let mut trashed = demo_item(NEXT_REVISION, "Compte factice");
            trashed.deleted_date = Some(NEXT_REVISION.to_owned());
            vec![Reply::Unit, item_reply(demo()), Reply::Unit, item_reply(trashed)]
        }
        _ => Vec::new(),
    }
}
