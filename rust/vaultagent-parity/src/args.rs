//! Lecture des arguments de la ligne de commande.

use crate::error::{ParityError, Result};

const DEFAULT_SERVER: &str = "https://vaultagent-test-vw.test";
const DEFAULT_EMAIL: &str = "vaultagent-test2@example.invalid";
const DEFAULT_PHRASE: &str = "factice-vaultagent-0001";
const DEFAULT_OWNER: &str = "tester";
const DEFAULT_PAUSE: u64 = 0;

/// Un serveur à comparer.
#[derive(Debug, PartialEq, Eq)]
pub struct Target {
    /// Adresse `hôte:port` du tunnel.
    pub address: String,
    /// Origine HTTPS déclarée du service (`https://localhost:<port>`).
    pub origin: String,
}

/// Configuration de l'outil.
#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    /// Serveur Node de référence.
    pub node: Target,
    /// Serveur Rust à valider.
    pub rust: Target,
    /// URL `https://` du serveur Bitwarden de test.
    pub server: String,
    /// Courriel du compte factice.
    pub email: String,
    /// Mot de passe maître factice.
    pub password: String,
    /// Propriétaire des routes administrateur.
    pub owner: String,
    /// Pause entre le passage Node et le passage Rust, en secondes (limite de connexions de Vaultwarden).
    pub pause: u64,
    /// Affiche l'étape et le statut de chaque échange (jamais les corps).
    pub trace: bool,
}

/// Transforme `http://hôte:port` en cible.
///
/// Paramètres : l'URL de base.
/// Retour : la cible.
/// # Errors
///
/// `usage` si l'URL n'est pas `http://hôte:port`.
fn target(url: &str) -> Result<Target> {
    let address = url
        .strip_prefix("http://")
        .map_or("", |rest| rest.trim_end_matches('/'));
    let port = address
        .rsplit_once(':')
        .map(|(_, port)| port)
        .filter(|port| port.parse::<u16>().is_ok());
    match port {
        Some(port) => Ok(Target {
            address: address.to_owned(),
            origin: format!("https://localhost:{port}"),
        }),
        None => Err(ParityError::Usage(format!(
            "URL de base attendue sous la forme http://hôte:port, reçu {url}"
        ))),
    }
}

/// Analyse les arguments.
///
/// Paramètres : les arguments sans le nom du programme.
/// Retour : la configuration.
/// # Errors
///
/// `usage` si `--node` ou `--rust` manque, si un indicateur est inconnu ou sans valeur.
pub fn parse(arguments: &[String]) -> Result<Config> {
    let mut node = None;
    let mut rust = None;
    let mut server = DEFAULT_SERVER.to_owned();
    let mut email = DEFAULT_EMAIL.to_owned();
    let mut password = DEFAULT_PHRASE.to_owned();
    let mut owner = DEFAULT_OWNER.to_owned();
    let mut pause = DEFAULT_PAUSE;
    let mut trace = false;
    let mut iterator = arguments.iter();
    while let Some(flag) = iterator.next() {
        if flag == "--trace" {
            trace = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| ParityError::Usage(format!("{flag} demande une valeur")))?;
        match flag.as_str() {
            "--node" => node = Some(target(value)?),
            "--rust" => rust = Some(target(value)?),
            "--server" => server.clone_from(value),
            "--email" => email.clone_from(value),
            "--password" => password.clone_from(value),
            "--owner" => owner.clone_from(value),
            "--pause" => {
                pause = value
                    .parse()
                    .map_err(|_| ParityError::Usage("--pause attend un nombre de secondes".to_owned()))?;
            }
            other => return Err(ParityError::Usage(format!("indicateur inconnu : {other}"))),
        }
    }
    match (node, rust) {
        (Some(node), Some(rust)) => Ok(Config {
            node,
            rust,
            server,
            email,
            password,
            owner,
            pause,
            trace,
        }),
        _ => Err(ParityError::Usage("--node et --rust sont obligatoires".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convertit une liste de textes en arguments.
    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|text| (*text).to_owned()).collect()
    }

    #[test]
    fn parses_targets_and_defaults() {
        let config = parse(&args(&[
            "--node",
            "http://127.0.0.1:8391",
            "--rust",
            "http://127.0.0.1:8390/",
        ]))
        .unwrap();
        assert_eq!(config.node.address, "127.0.0.1:8391");
        assert_eq!(config.rust.origin, "https://localhost:8390");
        assert_eq!(config.owner, "tester");
        assert_eq!(config.pause, 0);
    }

    #[test]
    fn trace_flag_needs_no_value() {
        let config = parse(&args(&[
            "--trace",
            "--node",
            "http://127.0.0.1:1",
            "--rust",
            "http://127.0.0.1:2",
        ]))
        .unwrap();
        assert!(config.trace);
    }

    #[test]
    fn refuses_missing_or_unknown() {
        assert!(parse(&args(&["--node", "http://127.0.0.1:1"])).is_err());
        assert!(parse(&args(&["--node", "127.0.0.1:1", "--rust", "http://127.0.0.1:2"])).is_err());
        assert!(parse(&args(&["--bogus", "x"])).is_err());
        assert!(parse(&args(&["--node"])).is_err());
    }
}
