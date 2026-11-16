//! Fichiers servis au propriétaire : archives `/downloads/*` et feuille de style `/style.css`.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::context::Context;
use crate::front_response::style;
use crate::http::Response;
use crate::session::VERSION;

/// Corps d'une erreur à statut explicite.
#[derive(Serialize)]
struct Failure {
    error: &'static str,
}

/// Sert un fichier si le chemin en désigne un.
///
/// Paramètres : le contexte, le chemin sans requête (méthode `GET` déjà vérifiée).
/// Retour : `None` si le chemin n'est pas un fichier ; sinon la réponse.
#[must_use]
pub fn serve(context: &Context, path: &str) -> Option<Response> {
    if path == "/style.css" {
        return Some(style());
    }
    let (name, directory) = (download_name(path)?, context.downloads_directory.as_ref()?);
    Some(download(directory, &name))
}

/// Nom du fichier d'archive d'une route de téléchargement ; la liste est fermée, aucun autre nom n'existe.
fn download_name(path: &str) -> Option<String> {
    let name = match path {
        "/downloads/windows-client.zip" => format!("VaultAgent-Windows-Client-{VERSION}-x64.zip"),
        "/downloads/linux-client.tar.gz" => format!("VaultAgent-Linux-Client-{VERSION}.tar.gz"),
        "/downloads/windows-client.zip.sha256" => format!("VaultAgent-Windows-Client-{VERSION}-x64.zip.sha256"),
        "/downloads/linux-client.tar.gz.sha256" => format!("VaultAgent-Linux-Client-{VERSION}.tar.gz.sha256"),
        _ => return None,
    };
    Some(name)
}

/// Réponse d'une archive, ou `404 download-unavailable` si elle ne se lit pas.
fn download(directory: &Path, name: &str) -> Response {
    match fs::read(directory.join(name)) {
        Ok(body) => Response {
            status: 200,
            headers: vec![
                ("content-type", "application/octet-stream".to_owned()),
                ("content-disposition", format!("attachment; filename=\"{name}\"")),
                ("cache-control", "no-store".to_owned()),
                ("x-content-type-options", "nosniff".to_owned()),
            ],
            body,
        },
        Err(_) => Response::json(
            404,
            &Failure {
                error: "download-unavailable",
            },
        ),
    }
}
