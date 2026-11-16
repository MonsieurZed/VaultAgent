//! Emplacement du fichier de configuration : `--config`, ou `~/.config/vaultagent/client.json` sous Unix.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::ClientError;

/// Détermine le fichier de configuration.
///
/// Paramètres : le chemin donné par `--config`, et le dossier personnel (`HOME`) lu par l'appelant.
/// Retour : le chemin explicite s'il existe ; sinon, sous Unix, `<home>/.config/vaultagent/client.json` ;
/// sous Windows (pas de chemin par défaut avant DPAPI) ou sans dossier personnel, une erreur d'usage.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn resolve(explicit: Option<&Path>, home: Option<&OsStr>) -> Result<PathBuf, ClientError> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    let home = home.filter(|value| !value.is_empty());
    match (cfg!(unix), home) {
        (true, Some(home)) => Ok(Path::new(home).join(".config").join("vaultagent").join("client.json")),
        (true, None) => Err(ClientError::Usage("HOME is not set; use --config <file>.".to_owned())),
        (false, _) => Err(ClientError::Usage(
            "--config <file> is required on this platform.".to_owned(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_path_always_wins() {
        let path = resolve(Some(Path::new("custom.json")), None).unwrap();
        assert_eq!(path, PathBuf::from("custom.json"));
    }

    #[cfg(unix)]
    #[test]
    fn unix_default_is_under_home() {
        let path = resolve(None, Some(OsStr::new("/home/agent"))).unwrap();
        assert_eq!(path, PathBuf::from("/home/agent/.config/vaultagent/client.json"));
        assert_eq!(resolve(None, None).unwrap_err().code(), "usage");
    }

    #[cfg(not(unix))]
    #[test]
    fn without_unix_the_config_option_is_required() {
        let error = resolve(None, Some(OsStr::new("C:\\Users\\agent"))).unwrap_err();
        assert_eq!(error.code(), "usage");
    }
}
