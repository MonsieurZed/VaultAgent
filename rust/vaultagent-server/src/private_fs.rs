//! Fichiers et dossiers privés : modes Unix 0700 et 0600, isolés derrière `cfg(unix)` (sans effet ailleurs).

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

use crate::error::Result;

/// Crée un dossier (et ses parents manquants) en 0700 sous Unix, puis impose 0700 au dossier lui-même.
///
/// Paramètres : le chemin du dossier.
/// Retour : rien.
///
/// # Errors
///
/// L'erreur d'entrée-sortie de la création ou du changement de mode.
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    build_dir(path)?;
    restrict_dir(path)
}

/// Crée les dossiers manquants avec le mode 0700.
///
/// Paramètres : le chemin.
/// Retour : rien.
///
/// # Errors
///
/// L'erreur d'entrée-sortie de la création.
#[cfg(unix)]
fn build_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(path)
}

/// Crée les dossiers manquants.
///
/// Paramètres : le chemin.
/// Retour : rien.
///
/// # Errors
///
/// L'erreur d'entrée-sortie de la création.
#[cfg(not(unix))]
fn build_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
}

/// Impose le mode 0700 à un dossier.
///
/// Paramètres : le chemin.
/// Retour : rien.
///
/// # Errors
///
/// L'erreur d'entrée-sortie du changement de mode.
#[cfg(unix)]
fn restrict_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

/// Sans effet hors Unix.
///
/// Paramètres : le chemin.
/// Retour : rien.
///
/// # Errors
///
/// Jamais.
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn restrict_dir(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// Ouvre en écriture un fichier neuf, en exclusif, en 0600 sous Unix.
///
/// Paramètres : le chemin, qui ne doit pas exister.
/// Retour : le fichier ouvert.
///
/// # Errors
///
/// `AlreadyExists` si le fichier existe, ou toute autre erreur d'entrée-sortie.
pub fn create_new(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    private_mode(&mut options);
    options.open(path)
}

/// Demande le mode 0600 à l'ouverture.
///
/// Paramètres : les options à compléter.
#[cfg(unix)]
fn private_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

/// Sans effet hors Unix.
///
/// Paramètres : les options.
#[cfg(not(unix))]
fn private_mode(_options: &mut OpenOptions) {}

/// Refuse une clé dont le mode laisse un accès au groupe ou aux autres (Unix seulement).
///
/// Paramètres : le chemin de `machine.key`.
/// Retour : rien.
///
/// # Errors
///
/// `private-key-permissions` si un bit `0o077` est posé ; erreur d'entrée-sortie si les métadonnées échouent.
#[cfg(unix)]
pub fn ensure_private(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if std::fs::metadata(path)?.permissions().mode() & 0o077 != 0 {
        return Err(crate::error::ServerError::PrivateKeyPermissions);
    }
    Ok(())
}

/// Sans contrôle hors Unix (Windows n'a pas de bits de mode comparables).
///
/// Paramètres : le chemin.
/// Retour : rien.
///
/// # Errors
///
/// Jamais.
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
pub fn ensure_private(_path: &Path) -> Result<()> {
    Ok(())
}
