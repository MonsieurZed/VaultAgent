//! Signature élevée côté appelant : lance `vaultagent elevation-sign` avec élévation (invite UAC), relit la preuve
//! dans un fichier temporaire au nom aléatoire puis le supprime, même en cas d'échec.

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use vaultagent_core::encoding::base64_encode;
use vaultagent_core::{Entropy, SystemEntropy};

use crate::elevation_sign::ElevatedProof;
use crate::elevation_system::ElevationSystem;
use crate::error::ClientError;

/// Nom aléatoire du fichier de sortie : `vaultagent-<32 hexadécimaux>.txt`.
fn output_path(temp_dir: &Path) -> Result<PathBuf, ClientError> {
    let mut bytes = [0u8; 16];
    SystemEntropy
        .fill(&mut bytes)
        .map_err(|_| ClientError::Elevation("No randomness available.".to_owned()))?;
    let name = bytes.iter().fold(String::new(), |mut text, byte| {
        write!(text, "{byte:02x}").unwrap_or_default();
        text
    });
    Ok(temp_dir.join(format!("vaultagent-{name}.txt")))
}

/// Lit la preuve si le processus élevé a réussi.
fn read_proof(code: u32, output: &Path) -> Result<ElevatedProof, ClientError> {
    let refused = || ClientError::Elevation("The Windows validation was refused.".to_owned());
    if code != 0 {
        return Err(refused());
    }
    let text = fs::read_to_string(output).map_err(|_| refused())?;
    ElevatedProof::parse(&text)
}

/// Signe un message par un processus élevé : l'invite UAC est la preuve de présence.
///
/// Paramètres : le système, le dossier temporaire de l'utilisateur et le message à signer.
/// Retour : la clé publique d'élévation et la signature ; `ElevationCancelled` si l'invite est annulée, `Elevation`
/// si la validation est refusée, expirée ou si la réponse est invalide. Le fichier de sortie est supprimé dans tous
/// les cas.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn elevated_sign(
    system: &dyn ElevationSystem,
    temp_dir: &Path,
    message: &str,
) -> Result<ElevatedProof, ClientError> {
    let sid = system.current_sid()?;
    let output = output_path(temp_dir)?;
    let args = format!(
        "elevation-sign {sid} {} \"{}\"",
        base64_encode(message.as_bytes()),
        output.display()
    );
    let result = system.run_elevated(&args).and_then(|code| read_proof(code, &output));
    fs::remove_file(&output).unwrap_or_default();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elevation_sign::run_elevation_sign;
    use crate::fake_elevation::FakeElevation;
    use crate::test_dir::TestDir;
    use vaultagent_core::encoding::base64_decode;
    use vaultagent_core::{ErrorCode, verify_pss};

    const SID: &str = "S-1-5-21-1-2-3-1001";

    /// Nombre de fichiers du dossier de test.
    fn files(dir: &TestDir) -> usize {
        fs::read_dir(dir.path()).unwrap().count()
    }

    #[test]
    fn child_arguments_follow_the_dotnet_command_line() {
        let dir = TestDir::new();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let record = std::rc::Rc::clone(&seen);
        let system = FakeElevation::standard(
            SID,
            Box::new(move |args| {
                *record.borrow_mut() = args.to_owned();
                Ok(1)
            }),
        );
        assert!(elevated_sign(&system, dir.path(), "message").is_err());
        let line = seen.borrow().clone();
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        assert_eq!(&parts[..2], ["elevation-sign", SID]);
        assert_eq!(base64_decode(parts[2]).unwrap(), b"message");
        assert!(parts[3].starts_with('"') && parts[3].ends_with(".txt\""));
        assert!(parts[3].contains("vaultagent-"));
    }

    #[test]
    fn full_round_trip_with_a_simulated_elevated_child() {
        let dir = TestDir::new();
        let data = TestDir::new();
        let data_path = data.path().to_path_buf();
        let system = FakeElevation::standard(
            SID,
            Box::new(move |args| {
                let parts: Vec<&str> = args.splitn(4, ' ').collect();
                let output = PathBuf::from(parts[3].trim_matches('"'));
                run_elevation_sign(
                    &FakeElevation::elevated(SID),
                    Some(&data_path),
                    parts[1],
                    parts[2],
                    &output,
                )
                .map(|()| 0)
            }),
        );
        let proof = elevated_sign(&system, dir.path(), "[\"msg\"]").unwrap();
        verify_pss(&proof.public_key, "[\"msg\"]", &proof.signature, ErrorCode::InvalidKey).unwrap();
        assert_eq!(files(&dir), 0);
        assert_eq!(system.launches().len(), 1);
    }

    #[test]
    fn cancellation_and_refusal_leave_no_file() {
        let dir = TestDir::new();
        let cancelled = FakeElevation::standard(SID, Box::new(|_| Err(ClientError::ElevationCancelled)));
        assert_eq!(
            elevated_sign(&cancelled, dir.path(), "m").unwrap_err().code(),
            "elevation-cancelled"
        );
        let refused = FakeElevation::standard(SID, Box::new(|_| Ok(2)));
        assert_eq!(
            elevated_sign(&refused, dir.path(), "m").unwrap_err().code(),
            "elevation-failed"
        );
        assert_eq!(files(&dir), 0);
    }

    #[test]
    fn invalid_output_is_refused_and_deleted() {
        let dir = TestDir::new();
        let system = FakeElevation::standard(
            SID,
            Box::new(|args| {
                let output = args.rsplit(' ').next().unwrap_or("").trim_matches('"');
                fs::write(output, "only-one-line\n").unwrap();
                Ok(0)
            }),
        );
        assert_eq!(
            elevated_sign(&system, dir.path(), "m").unwrap_err().code(),
            "elevation-failed"
        );
        assert_eq!(files(&dir), 0);
    }
}
