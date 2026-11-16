//! Réglages de démarrage lus dans les variables d'environnement de `linux/server.mjs`, plus l'adresse d'écoute.

use std::path::PathBuf;

use vaultagent_core::https_origin;

use crate::error::{Result, ServerError};

const DEFAULT_DATA: &str = "/data";
const DEFAULT_BIND: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_WORKERS: usize = 8;
const MAX_WORKERS: usize = 64;

/// Réglages du service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Dossier des données (`VAULTAGENT_DATA`, `/data` par défaut).
    pub data: PathBuf,
    /// Origine publique HTTPS exacte (`VAULTAGENT_ORIGIN`).
    pub origin: String,
    /// Adresse IP du proxy de confiance (`VAULTAGENT_TRUSTED_PROXY`).
    pub trusted_proxy: String,
    /// Nom Authentik du propriétaire (`VAULTAGENT_OWNER`).
    pub owner: String,
    /// Dossier des téléchargements (`VAULTAGENT_DOWNLOADS`), absent si la variable est vide.
    pub downloads: Option<PathBuf>,
    /// Adresse d'écoute (`VAULTAGENT_BIND`, `0.0.0.0` par défaut).
    pub bind: String,
    /// Port d'écoute (`VAULTAGENT_PORT`, 8080 par défaut).
    pub port: u16,
    /// Nombre de fils de travail (`VAULTAGENT_WORKERS`, 8 par défaut, 1 à 64).
    pub workers: usize,
}

impl Settings {
    /// Construit les réglages depuis une fonction de lecture des variables.
    ///
    /// Paramètres : la fonction qui rend la valeur d'une variable, absente ou non.
    /// Retour : les réglages.
    ///
    /// # Errors
    ///
    /// `Settings` si l'origine n'est pas une origine HTTPS exacte, si le proxy ou le propriétaire manque, ou si le
    /// port ou le nombre de fils n'est pas un nombre valide.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let non_empty = |name: &str| get(name).filter(|value| !value.is_empty());
        let origin = non_empty("VAULTAGENT_ORIGIN")
            .filter(|value| https_origin(value).as_deref() == Some(value.as_str()))
            .ok_or_else(|| ServerError::Settings("VAULTAGENT_ORIGIN : origine HTTPS exacte requise".to_owned()))?;
        let (Some(trusted_proxy), Some(owner)) = (non_empty("VAULTAGENT_TRUSTED_PROXY"), non_empty("VAULTAGENT_OWNER"))
        else {
            return Err(ServerError::Settings(
                "VAULTAGENT_TRUSTED_PROXY et VAULTAGENT_OWNER sont requis".to_owned(),
            ));
        };
        let workers = number(non_empty("VAULTAGENT_WORKERS"), DEFAULT_WORKERS, "VAULTAGENT_WORKERS")?;
        if !(1..=MAX_WORKERS).contains(&workers) {
            return Err(ServerError::Settings(
                "VAULTAGENT_WORKERS : entre 1 et 64 attendu".to_owned(),
            ));
        }
        Ok(Self {
            data: PathBuf::from(non_empty("VAULTAGENT_DATA").unwrap_or_else(|| DEFAULT_DATA.to_owned())),
            origin,
            trusted_proxy,
            owner,
            downloads: non_empty("VAULTAGENT_DOWNLOADS").map(PathBuf::from),
            bind: non_empty("VAULTAGENT_BIND").unwrap_or_else(|| DEFAULT_BIND.to_owned()),
            port: number(non_empty("VAULTAGENT_PORT"), DEFAULT_PORT, "VAULTAGENT_PORT")?,
            workers,
        })
    }
}

/// Lit un nombre facultatif.
///
/// Paramètres : le texte, la valeur par défaut, le nom de la variable pour le message.
/// Retour : le nombre.
fn number<T: std::str::FromStr>(text: Option<String>, default: T, name: &str) -> Result<T> {
    match text {
        None => Ok(default),
        Some(value) => value
            .parse()
            .map_err(|_| ServerError::Settings(format!("{name} : nombre invalide"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fonction de lecture des variables à partir d'une liste fixe.
    fn lookup(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    const BASE: [(&str, &str); 3] = [
        ("VAULTAGENT_ORIGIN", "https://vaultagent.example.test"),
        ("VAULTAGENT_TRUSTED_PROXY", "10.0.0.2"),
        ("VAULTAGENT_OWNER", "proprietaire"),
    ];

    #[test]
    fn defaults_follow_the_node_server() {
        let settings = Settings::from_lookup(lookup(&BASE)).unwrap();
        assert_eq!(settings.data, PathBuf::from("/data"));
        assert_eq!(
            (settings.port, settings.workers, settings.bind.as_str()),
            (8080, 8, "0.0.0.0")
        );
        assert_eq!(settings.downloads, None);
    }

    #[test]
    fn origin_proxy_owner_and_numbers_are_checked() {
        let bad_origin: &'static [(&str, &str)] = &[
            ("VAULTAGENT_ORIGIN", "https://vaultagent.example.test/"),
            ("VAULTAGENT_TRUSTED_PROXY", "10.0.0.2"),
            ("VAULTAGENT_OWNER", "o"),
        ];
        assert!(Settings::from_lookup(lookup(bad_origin)).is_err());
        let no_owner: &'static [(&str, &str)] = &[BASE[0], BASE[1]];
        assert!(Settings::from_lookup(lookup(no_owner)).is_err());
        let bad_port: &'static [(&str, &str)] = &[BASE[0], BASE[1], BASE[2], ("VAULTAGENT_PORT", "http")];
        assert!(Settings::from_lookup(lookup(bad_port)).is_err());
        let zero: &'static [(&str, &str)] = &[BASE[0], BASE[1], BASE[2], ("VAULTAGENT_WORKERS", "0")];
        assert!(Settings::from_lookup(lookup(zero)).is_err());
    }
}
