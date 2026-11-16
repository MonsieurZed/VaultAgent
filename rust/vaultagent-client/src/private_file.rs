//! Fichiers privés : contrôle des permissions (Unix) et écriture atomique en `0600` dans un dossier `0700`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::ClientError;

/// Erreur de configuration à partir d'une erreur d'entrée-sortie, sans chemin ni contenu.
fn io_error(action: &str, error: &std::io::Error) -> ClientError {
    ClientError::Config(format!("Cannot {action} the client configuration ({}).", error.kind()))
}

/// Uid du processus courant, lu sans `unsafe` : le propriétaire de `/proc/self` (Linux).
#[cfg(unix)]
fn process_owner() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    fs::metadata("/proc/self").ok().map(|details| details.uid())
}

/// Refuse un fichier lisible par un autre utilisateur ou appartenant à un autre propriétaire.
///
/// Paramètres : le chemin du fichier.
/// Retour : `Ok` si le fichier est privé ; sous Unix, `Config` si `mode & 0o077` n'est pas nul ou si le
/// propriétaire n'est pas l'utilisateur courant (contrôle fait quand `/proc/self` est lisible). Sous Windows,
/// aucun contrôle (le stockage protégé par DPAPI arrive en phase 4).
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn check_private(path: &Path) -> Result<(), ClientError> {
    let details = fs::metadata(path).map_err(|error| io_error("read", &error))?;
    is_private(&details).then_some(()).ok_or_else(|| {
        ClientError::Config("Client configuration must be private (mode 0600, owned by you).".to_owned())
    })
}

/// Indique si les métadonnées décrivent un fichier privé : sans accès de groupe ni d'autrui, propriétaire courant.
#[cfg(unix)]
fn is_private(details: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    details.mode() & 0o077 == 0 && process_owner().is_none_or(|uid| uid == details.uid())
}

/// Sans Unix, tout fichier est accepté.
#[cfg(not(unix))]
fn is_private(_details: &fs::Metadata) -> bool {
    true
}

/// Crée le dossier parent en `0700` s'il manque.
fn create_parent(parent: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(parent)
}

/// Ouvre un nouveau fichier temporaire en `0600`.
fn open_private(path: &Path) -> std::io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// Nom du fichier temporaire voisin de la cible.
fn temporary_path(target: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut name = target
        .file_name()
        .map_or_else(std::ffi::OsString::new, std::ffi::OsStr::to_os_string);
    name.push(format!(".{}.{nanos}.tmp", std::process::id()));
    target.with_file_name(name)
}

/// Écrit le fichier temporaire puis le renomme sur la cible.
fn write_then_rename(temporary: &Path, target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = open_private(temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, target)
}

/// Écrit un fichier de façon atomique : dossier `0700`, temporaire `0600`, puis `rename`.
///
/// Paramètres : le chemin cible et le contenu.
/// Retour : `Ok`, ou `Config` si le dossier, l'écriture ou le renommage échoue (le temporaire est alors supprimé).
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), ClientError> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        create_parent(parent).map_err(|error| io_error("create the folder of", &error))?;
    }
    let temporary = temporary_path(path);
    let outcome = write_then_rename(&temporary, path, bytes);
    if outcome.is_err() {
        fs::remove_file(&temporary).unwrap_or_default();
    }
    outcome.map_err(|error| io_error("write", &error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir::TestDir;

    #[test]
    fn atomic_write_replaces_the_target_and_leaves_no_temporary() {
        let dir = TestDir::new();
        let target = dir.path().join("sub").join("client.json");
        write_private_atomic(&target, b"first").unwrap();
        write_private_atomic(&target, b"second").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"second");
        let names: Vec<_> = fs::read_dir(target.parent().unwrap()).unwrap().collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn missing_file_is_a_config_error() {
        let dir = TestDir::new();
        let error = check_private(&dir.path().join("absent.json")).unwrap_err();
        assert_eq!(error.code(), "config");
    }

    #[cfg(unix)]
    #[test]
    fn written_file_is_0600_and_folder_0700() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TestDir::new();
        let target = dir.path().join("vaultagent").join("client.json");
        write_private_atomic(&target, b"{}").unwrap();
        let file_mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        let dir_mode = fs::metadata(target.parent().unwrap()).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);
        check_private(&target).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn group_or_world_access_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TestDir::new();
        let target = dir.path().join("client.json");
        fs::write(&target, b"{}").unwrap();
        for mode in [0o640, 0o604, 0o660, 0o644] {
            fs::set_permissions(&target, fs::Permissions::from_mode(mode)).unwrap();
            assert!(check_private(&target).is_err(), "mode {mode:o} must be refused");
        }
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        check_private(&target).unwrap();
    }
}
