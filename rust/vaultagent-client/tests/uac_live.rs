//! Essais réels de la validation Windows : chaque test fait apparaître une invite UAC sur l'écran. Ils sont ignorés
//! par défaut ; à lancer avec Zed : `cargo test -p vaultagent-client --test uac_live -- --ignored --test-threads=1`.
//! La clé d'élévation est écrite dans un dossier de test, jamais dans le vrai `%ProgramData%\VaultAgent`.

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use vaultagent_client::elevated_sign::elevated_sign;
use vaultagent_client::elevation_key::elevation_key_path;
use vaultagent_client::elevation_system::ElevationSystem;
use vaultagent_client::error::ClientError;
use vaultagent_client::protector::Protector;
use vaultagent_client::windows_elevation::WindowsElevation;
use vaultagent_core::{ErrorCode, verify_pss};
use vaultagent_win32::Win32Error;

/// Système réel dont le processus élevé est le binaire `vaultagent` du test, avec un dossier de clé de test.
struct LiveElevation {
    program_data: PathBuf,
}

impl ElevationSystem for LiveElevation {
    /// Délègue au système Windows réel.
    fn is_elevated(&self) -> bool {
        WindowsElevation.is_elevated()
    }

    /// Délègue au système Windows réel.
    fn current_sid(&self) -> Result<String, ClientError> {
        WindowsElevation.current_sid()
    }

    /// Délègue au système Windows réel.
    fn create_private_directory(&self, path: &Path) -> Result<(), ClientError> {
        WindowsElevation.create_private_directory(path)
    }

    /// Délègue au système Windows réel.
    fn machine_protector(&self) -> Result<Box<dyn Protector>, ClientError> {
        WindowsElevation.machine_protector()
    }

    /// Lance le binaire `vaultagent` du test avec élévation et le dossier de clé de test.
    fn run_elevated(&self, args: &str) -> Result<u32, ClientError> {
        let line = format!("{args} --program-data \"{}\"", self.program_data.display());
        let exe = PathBuf::from(env!("CARGO_BIN_EXE_vaultagent"));
        vaultagent_win32::run_elevated(&exe, &line, Duration::from_secs(120)).map_err(|error| match error {
            Win32Error::Cancelled => ClientError::ElevationCancelled,
            other => ClientError::Elevation(other.to_string()),
        })
    }
}

/// Dossier de test supprimé à la fin ; la clé protégée y reste lisible par l'utilisateur seulement après coup.
fn test_base() -> PathBuf {
    let base = std::env::temp_dir().join(format!("vaultagent-uac-live-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    base
}

#[test]
#[ignore = "invite UAC : à lancer avec Zed, cliquer Oui"]
fn real_elevated_signature_is_accepted_by_the_core_verifier() {
    let base = test_base();
    let system = LiveElevation {
        program_data: base.clone(),
    };
    let message = "[\"VaultAgent.Unlock.v2\",\"https://uac-live.test\",\"default\",\"r\",\"c\",true,false]";
    let proof = elevated_sign(&system, &std::env::temp_dir(), message).unwrap();
    verify_pss(&proof.public_key, message, &proof.signature, ErrorCode::InvalidKey).unwrap();
    let again = elevated_sign(&system, &std::env::temp_dir(), message).unwrap();
    assert_eq!(again.public_key, proof.public_key);
    assert!(elevation_key_path(&base, "S-1-5-0").starts_with(&base));
}

#[test]
#[ignore = "invite UAC : à lancer avec Zed, cliquer Non"]
fn cancelled_prompt_is_refused_with_the_dedicated_error() {
    let system = LiveElevation {
        program_data: test_base(),
    };
    let error = elevated_sign(&system, &std::env::temp_dir(), "[\"annule\"]").unwrap_err();
    assert_eq!(error.code(), "elevation-cancelled");
}
