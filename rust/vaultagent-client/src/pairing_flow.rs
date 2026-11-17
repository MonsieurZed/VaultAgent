//! Association complète : clé d'approbateur facultative, demande approuvée sur le serveur, enregistrement protégé.
//! Partagée par `pair-request` et par la fenêtre d'association de l'application de barre.

use std::io::Write;

use crate::config::ClientConfig;
use crate::config_store::ConfigStore;
use crate::error::ClientError;
use crate::pairing::pair_request;
use crate::pause::Pause;
use crate::server_address::ServerAddress;
use crate::transport::Transport;

/// Associe ce client au serveur et enregistre la configuration.
///
/// Paramètres : le stockage, le transport vers le serveur, l'adresse, le nom de l'appareil, vrai pour enregistrer la
/// clé d'approbateur de ce poste, la pause entre deux interrogations et la sortie du code de vérification.
/// Retour : l'identifiant du coffre associé, s'il est donné.
///
/// # Errors
///
/// Clé d'approbateur illisible, association refusée ou expirée, serveur injoignable, enregistrement impossible.
pub fn pair_and_save(
    store: &ConfigStore,
    transport: &dyn Transport,
    server: ServerAddress,
    device: &str,
    approver: bool,
    pause: &mut dyn Pause,
    notice: &mut dyn Write,
) -> Result<Option<String>, ClientError> {
    let approver_key = approver.then(|| store.approver_public_key()).transpose()?;
    let paired = pair_request(transport, &server, device, approver_key.as_deref(), pause, notice)?;
    store.save(&ClientConfig {
        server,
        token: paired.token,
    })?;
    Ok(paired.vault_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_transport::FakeTransport;
    use crate::pause::RecordedPause;
    use crate::test_dir::{TestDir, sample_token};

    #[test]
    fn approved_pairing_is_saved_and_the_code_is_announced() {
        let dir = TestDir::new();
        let store = ConfigStore::locate(Some(&dir.path().join("client.json")), None, None).unwrap();
        let approved = format!(
            "{{\"state\":\"approved\",\"token\":\"{}\",\"vaultId\":\"v1\"}}",
            sample_token()
        );
        let transport = FakeTransport::new()
            .reply(
                200,
                "{\"requestId\":\"r\",\"secret\":\"s\",\"verification\":\"ABCD-EFGH\",\"expiresInSeconds\":6}",
            )
            .reply(200, &approved);
        let server = ServerAddress::parse("https://vault.example.test").unwrap();
        let (mut pause, mut notice) = (RecordedPause::default(), Vec::new());
        let vault = pair_and_save(&store, &transport, server, "PC", false, &mut pause, &mut notice).unwrap();
        assert_eq!(vault.as_deref(), Some("v1"));
        assert!(String::from_utf8(notice).unwrap().contains("ABCD-EFGH"));
        assert_eq!(store.load().unwrap().server.as_str(), "https://vault.example.test");
    }

    #[test]
    fn denied_pairing_saves_nothing() {
        let dir = TestDir::new();
        let store = ConfigStore::locate(Some(&dir.path().join("client.json")), None, None).unwrap();
        let transport = FakeTransport::new()
            .reply(
                200,
                "{\"requestId\":\"r\",\"secret\":\"s\",\"verification\":\"A\",\"expiresInSeconds\":6}",
            )
            .reply(200, "{\"state\":\"denied\"}");
        let server = ServerAddress::parse("https://vault.example.test").unwrap();
        let (mut pause, mut notice) = (RecordedPause::default(), Vec::new());
        let result = pair_and_save(&store, &transport, server, "PC", false, &mut pause, &mut notice);
        assert_eq!(result.unwrap_err(), ClientError::PairingDenied);
        assert!(store.load().is_err());
    }
}
