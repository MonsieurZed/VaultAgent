//! Clé d'élévation : RSA 2048 PKCS#8 protégé par DPAPI machine, dans `%ProgramData%\VaultAgent\elevation\<sid>.key`,
//! au même endroit et au même format que le client .NET. Seul un processus administrateur peut la lire.

use std::fs;
use std::path::{Path, PathBuf};

use crate::approver_key::ApproverKey;
use crate::elevation_system::ElevationSystem;
use crate::error::ClientError;

/// Dossier de l'application sous `%ProgramData%`.
const APPLICATION_FOLDER: &str = "VaultAgent";

/// Sous-dossier réservé aux administrateurs.
const ELEVATION_FOLDER: &str = "elevation";

/// Indique si un texte a la forme d'un SID (`S-1-5-21-…`), donc sûr comme nom de fichier.
///
/// Paramètres : le texte reçu sur la ligne de commande.
/// Retour : `true` si le texte n'est composé que de `S`, de chiffres et de tirets.
#[must_use]
pub fn is_valid_sid(sid: &str) -> bool {
    sid.starts_with("S-")
        && sid.len() <= 184
        && sid
            .bytes()
            .all(|byte| byte == b'S' || byte == b'-' || byte.is_ascii_digit())
}

/// Chemin de la clé d'élévation d'un utilisateur.
///
/// Paramètres : le dossier de base (`%ProgramData%`) et le SID de l'utilisateur.
/// Retour : `<base>\VaultAgent\elevation\<sid>.key`.
#[must_use]
pub fn elevation_key_path(program_data: &Path, sid: &str) -> PathBuf {
    program_data
        .join(APPLICATION_FOLDER)
        .join(ELEVATION_FOLDER)
        .join(format!("{sid}.key"))
}

/// Prépare le dossier protégé de la clé : `VaultAgent` en héritage normal, `elevation` réservé aux administrateurs.
fn ensure_directory(system: &dyn ElevationSystem, key_path: &Path) -> Result<(), ClientError> {
    let Some(directory) = key_path.parent() else {
        return Err(ClientError::Config("Invalid elevation key path.".to_owned()));
    };
    if let Some(application) = directory.parent() {
        fs::create_dir_all(application)
            .map_err(|_| ClientError::Config("Cannot create the application data folder.".to_owned()))?;
    }
    system.create_private_directory(directory)
}

/// Charge la clé d'élévation d'un utilisateur, ou la crée si elle manque (sans jamais écraser une clé existante).
///
/// Paramètres : le système, le dossier de base (`%ProgramData%`) et le SID de l'utilisateur.
/// Retour : la clé, ou `Usage` si le SID est invalide, ou `Config` si le dossier, le fichier ou DPAPI échouent.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn load_or_create(
    system: &dyn ElevationSystem,
    program_data: &Path,
    sid: &str,
) -> Result<ApproverKey, ClientError> {
    if !is_valid_sid(sid) {
        return Err(ClientError::Usage("Invalid user identifier.".to_owned()));
    }
    let path = elevation_key_path(program_data, sid);
    ensure_directory(system, &path)?;
    ApproverKey::load_or_create(&path, system.machine_protector()?.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_elevation::FakeElevation;
    use crate::test_dir::TestDir;

    const SID: &str = "S-1-5-21-1-2-3-1001";

    #[test]
    fn path_matches_the_dotnet_layout() {
        let path = elevation_key_path(Path::new("base"), SID);
        assert_eq!(
            path,
            Path::new("base")
                .join("VaultAgent")
                .join("elevation")
                .join("S-1-5-21-1-2-3-1001.key")
        );
    }

    #[test]
    fn sid_shape_is_checked_against_path_tricks() {
        assert!(is_valid_sid(SID));
        for bad in ["", "S", "..\\x", "S-1/../2", "S-1-5 21", "abc", "S-1-5-21-é"] {
            assert!(!is_valid_sid(bad), "{bad}");
        }
    }

    #[test]
    fn key_is_created_once_and_reloaded_with_the_same_public_key() {
        let dir = TestDir::new();
        let system = FakeElevation::elevated(SID);
        let first = load_or_create(&system, dir.path(), SID).unwrap().public_key().unwrap();
        let again = load_or_create(&system, dir.path(), SID).unwrap().public_key().unwrap();
        assert_eq!(first, again);
        let file = elevation_key_path(dir.path(), SID);
        assert!(fs::read(file).unwrap().len() > 1000);
        assert_eq!(fs::read_dir(file_folder(dir.path())).unwrap().count(), 1);
    }

    #[test]
    fn invalid_sid_creates_nothing() {
        let dir = TestDir::new();
        let error = load_or_create(&FakeElevation::elevated(SID), dir.path(), "../x")
            .err()
            .unwrap();
        assert_eq!(error.code(), "usage");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn damaged_key_file_is_refused_and_kept() {
        let dir = TestDir::new();
        let system = FakeElevation::elevated(SID);
        load_or_create(&system, dir.path(), SID).unwrap();
        let file = elevation_key_path(dir.path(), SID);
        fs::write(&file, b"damaged").unwrap();
        assert_eq!(load_or_create(&system, dir.path(), SID).err().unwrap().code(), "config");
        assert_eq!(fs::read(file).unwrap(), b"damaged");
    }

    #[cfg(windows)]
    #[test]
    fn machine_dpapi_key_round_trips_in_a_temporary_folder() {
        use crate::approver_key::ApproverKey;
        use crate::protector::MachineDpapiProtector;
        let dir = TestDir::new();
        let path = dir.path().join("test.key");
        let first = ApproverKey::load_or_create(&path, &MachineDpapiProtector).unwrap();
        let again = ApproverKey::load_or_create(&path, &MachineDpapiProtector).unwrap();
        assert_eq!(first.public_key().unwrap(), again.public_key().unwrap());
        assert!(fs::read(&path).unwrap().len() > 500);
    }

    /// Dossier qui contient le fichier de clé.
    fn file_folder(base: &Path) -> PathBuf {
        elevation_key_path(base, SID).parent().unwrap().to_path_buf()
    }
}
