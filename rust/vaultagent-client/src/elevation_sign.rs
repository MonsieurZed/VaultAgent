//! Commande cachée `elevation-sign <sid> <message-base64> <fichier-sortie>` : exécutée par le processus élevé, elle
//! signe le message avec la clé d'élévation et écrit deux lignes (SPKI base64, signature RSA-PSS base64).

use std::fs;
use std::path::Path;

use vaultagent_core::encoding::base64_decode;

use crate::elevation_key::load_or_create;
use crate::elevation_system::ElevationSystem;
use crate::error::ClientError;

/// Preuve d'élévation : clé publique enrôlée et signature du message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevatedProof {
    /// Clé publique d'élévation (SPKI DER en base64).
    pub public_key: String,
    /// Signature RSA-PSS SHA-256 du message (base64).
    pub signature: String,
}

impl ElevatedProof {
    /// Lit la preuve écrite par le processus élevé.
    ///
    /// Paramètres : le contenu du fichier de sortie.
    /// Retour : la preuve, ou `Elevation` si le fichier n'a pas exactement deux lignes non vides.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn parse(text: &str) -> Result<Self, ClientError> {
        let lines: Vec<&str> = text.lines().collect();
        match lines.as_slice() {
            [public_key, signature] if !public_key.is_empty() && !signature.is_empty() => Ok(Self {
                public_key: (*public_key).to_owned(),
                signature: (*signature).to_owned(),
            }),
            _ => Err(ClientError::Elevation("Invalid elevated answer.".to_owned())),
        }
    }

    /// Texte du fichier de sortie : deux lignes, comme `File.WriteAllLines`.
    ///
    /// Retour : `<clé publique>\n<signature>\n`.
    #[must_use]
    pub fn to_file_text(&self) -> String {
        format!("{}\n{}\n", self.public_key, self.signature)
    }
}

/// Signe un message avec la clé d'élévation (créée si elle manque) ; refuse hors d'un processus élevé.
///
/// Paramètres : le système, le dossier `%ProgramData%`, le SID, le message en base64 et le fichier de sortie.
/// Retour : `ElevationRequired` sans élévation (rien n'est lu ni écrit), `Usage` pour un SID ou un message invalide,
/// `Config` si la clé ou le fichier de sortie échouent.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn run_elevation_sign(
    system: &dyn ElevationSystem,
    program_data: Option<&Path>,
    sid: &str,
    message_base64: &str,
    output: &Path,
) -> Result<(), ClientError> {
    if !system.is_elevated() {
        return Err(ClientError::ElevationRequired);
    }
    let program_data = program_data.ok_or_else(|| ClientError::Usage("No program data folder.".to_owned()))?;
    let message = base64_decode(message_base64)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or_else(|| ClientError::Usage("The message is not valid base64 text.".to_owned()))?;
    let key = load_or_create(system, program_data, sid)?;
    let proof = ElevatedProof {
        public_key: key.public_key()?,
        signature: key.sign(&message)?,
    };
    fs::write(output, proof.to_file_text())
        .map_err(|_| ClientError::Config("Cannot write the elevated answer.".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_elevation::FakeElevation;
    use crate::test_dir::TestDir;
    use vaultagent_core::encoding::base64_encode;
    use vaultagent_core::{ErrorCode, verify_pss};

    const SID: &str = "S-1-5-21-1-2-3-1001";

    #[test]
    fn refused_without_elevation_and_nothing_is_written() {
        let dir = TestDir::new();
        let output = dir.path().join("out.txt");
        let system = FakeElevation::standard(SID, Box::new(|_| Ok(0)));
        let error = run_elevation_sign(&system, Some(dir.path()), SID, &base64_encode(b"m"), &output).unwrap_err();
        assert_eq!(error.code(), "elevation-required");
        assert_eq!(error.exit_code().code(), 2);
        assert!(!output.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn writes_two_lines_that_the_core_verifier_accepts() {
        let dir = TestDir::new();
        let output = dir.path().join("out.txt");
        let system = FakeElevation::elevated(SID);
        let message = "[\"VaultAgent.Unlock.v2\",\"https://x.test\",\"default\",\"r\",\"c\",true,false]";
        run_elevation_sign(
            &system,
            Some(dir.path()),
            SID,
            &base64_encode(message.as_bytes()),
            &output,
        )
        .unwrap();
        let text = std::fs::read_to_string(&output).unwrap();
        assert_eq!(text.lines().count(), 2);
        let proof = ElevatedProof::parse(&text).unwrap();
        verify_pss(&proof.public_key, message, &proof.signature, ErrorCode::InvalidKey).unwrap();
        assert!(verify_pss(&proof.public_key, "autre", &proof.signature, ErrorCode::InvalidKey).is_err());
    }

    #[test]
    fn invalid_message_or_sid_is_a_usage_error() {
        let dir = TestDir::new();
        let output = dir.path().join("out.txt");
        let system = FakeElevation::elevated(SID);
        for (sid, message) in [(SID, "***"), ("../x", "bQ==")] {
            let error = run_elevation_sign(&system, Some(dir.path()), sid, message, &output).unwrap_err();
            assert_eq!(error.code(), "usage");
        }
        assert!(!output.exists());
    }

    #[test]
    fn parse_accepts_crlf_and_refuses_other_shapes() {
        let proof = ElevatedProof::parse("a\r\nb\r\n").unwrap();
        assert_eq!((proof.public_key.as_str(), proof.signature.as_str()), ("a", "b"));
        for bad in ["", "a\n", "a\nb\nc\n", "\nb\n"] {
            assert_eq!(
                ElevatedProof::parse(bad).unwrap_err().code(),
                "elevation-failed",
                "{bad:?}"
            );
        }
    }
}
