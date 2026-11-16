//! Nom de la machine pour `pair-request`, lu sans dépendance.

/// Nom utilisé quand aucune source ne donne rien.
pub const FALLBACK_NAME: &str = "vaultagent-client";

/// Détermine le nom de la machine.
///
/// Paramètres : le contenu de `/etc/hostname` s'il a pu être lu, puis une fonction de lecture de variable
/// d'environnement (`HOSTNAME`, `COMPUTERNAME`).
/// Retour : la première source non vide (espaces retirés), ou `vaultagent-client`.
pub fn machine_name(hostname_file: Option<String>, env: impl Fn(&str) -> Option<String>) -> String {
    hostname_file
        .into_iter()
        .chain(env("HOSTNAME"))
        .chain(env("COMPUTERNAME"))
        .map(|text| text.trim().to_owned())
        .find(|text| !text.is_empty())
        .unwrap_or_else(|| FALLBACK_NAME.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_wins_then_environment_then_fallback() {
        let env = |name: &str| match name {
            "HOSTNAME" => Some("from-env".to_owned()),
            "COMPUTERNAME" => Some("WIN-BOX".to_owned()),
            _ => None,
        };
        assert_eq!(machine_name(Some("agents-vm\n".to_owned()), env), "agents-vm");
        assert_eq!(machine_name(None, env), "from-env");
        assert_eq!(machine_name(Some("  \n".to_owned()), env), "from-env");
        let only_windows = |name: &str| (name == "COMPUTERNAME").then(|| "TOWERZ".to_owned());
        assert_eq!(machine_name(None, only_windows), "TOWERZ");
        assert_eq!(machine_name(None, |_| None), FALLBACK_NAME);
    }
}
