//! Commandes de lecture : `status`, `vaults`, `sharing`, `catalog` et `preview`.

use serde::Serialize;
use vaultagent_core::Uuid;

use crate::api::agent_call;
use crate::error::ClientError;
use crate::token::Token;
use crate::transport::Transport;

/// Commande de lecture, sans effet sur le coffre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadCommand {
    /// État du coffre et de la demande de déverrouillage.
    Status,
    /// Coffres autorisés pour le jeton.
    Vaults,
    /// Organisations et collections accessibles.
    Sharing,
    /// Catalogue des fiches, filtré par une recherche facultative.
    Catalog(Option<String>),
    /// Liste des dossiers et fiches, ou détail d'une fiche.
    Preview(Option<String>),
}

/// Corps commun des routes de lecture.
#[derive(Serialize)]
struct ReadBody<'a> {
    #[serde(rename = "vaultId", skip_serializing_if = "Option::is_none")]
    vault_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search: Option<&'a str>,
    #[serde(rename = "itemId", skip_serializing_if = "Option::is_none")]
    item_id: Option<&'a str>,
}

impl ReadCommand {
    /// Nom de la route agent appelée.
    ///
    /// Retour : `status`, `vaults`, `sharing`, `catalog` ou `write-preview`.
    #[must_use]
    pub fn route(&self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Vaults => "vaults",
            Self::Sharing => "sharing",
            Self::Catalog(_) => "catalog",
            Self::Preview(_) => "write-preview",
        }
    }

    /// Valide les arguments avant tout appel réseau.
    fn validate(&self, vault: Option<&str>) -> Result<(), ClientError> {
        if vault.is_some_and(str::is_empty) {
            return Err(ClientError::Usage("Vault id required after --vault.".to_owned()));
        }
        match self {
            Self::Preview(Some(id)) if Uuid::parse(id).is_none() => {
                Err(ClientError::Usage("Item id must be a UUID.".to_owned()))
            }
            _ => Ok(()),
        }
    }

    /// Exécute la lecture.
    ///
    /// Paramètres : le transport, le jeton, le coffre choisi (`--vault`).
    /// Retour : le corps JSON du serveur sur une ligne, ou `Usage` pour un argument invalide, ou l'erreur classée
    /// (`locked` donne le code de sortie 4, `vault-required` un refus).
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn run(&self, transport: &dyn Transport, token: &Token, vault: Option<&str>) -> Result<String, ClientError> {
        self.validate(vault)?;
        let body = ReadBody {
            vault_id: vault,
            search: match self {
                Self::Catalog(search) => Some(search.as_deref().unwrap_or("")),
                _ => None,
            },
            item_id: match self {
                Self::Preview(id) => id.as_deref(),
                _ => None,
            },
        };
        agent_call(transport, token, self.route(), &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_transport::FakeTransport;
    use crate::test_dir::sample_token;

    const ITEM: &str = "0000000a-0000-4000-8000-00000000000c";

    /// Aide de test.
    fn token() -> Token {
        Token::parse(&sample_token()).unwrap()
    }

    #[test]
    fn each_command_calls_its_route_with_the_expected_body() {
        let cases = [
            (ReadCommand::Status, None, "status", "{}"),
            (ReadCommand::Vaults, None, "vaults", "{}"),
            (ReadCommand::Sharing, Some("work"), "sharing", "{\"vaultId\":\"work\"}"),
            (ReadCommand::Catalog(None), None, "catalog", "{\"search\":\"\"}"),
            (
                ReadCommand::Catalog(Some("mail".to_owned())),
                Some("work"),
                "catalog",
                "{\"vaultId\":\"work\",\"search\":\"mail\"}",
            ),
            (ReadCommand::Preview(None), None, "write-preview", "{}"),
            (
                ReadCommand::Preview(Some(ITEM.to_owned())),
                None,
                "write-preview",
                &format!("{{\"itemId\":\"{ITEM}\"}}"),
            ),
        ];
        for (command, vault, route, body) in cases {
            let transport = FakeTransport::new().reply(200, "[]");
            assert_eq!(command.run(&transport, &token(), vault).unwrap(), "[]");
            let sent = &transport.calls()[0];
            assert_eq!(sent.path, format!("/api/agent/{route}"));
            assert_eq!(sent.body, body);
        }
    }

    #[test]
    fn invalid_arguments_never_reach_the_network() {
        let transport = FakeTransport::new();
        let bad_id = ReadCommand::Preview(Some("not-a-uuid".to_owned()));
        assert_eq!(bad_id.run(&transport, &token(), None).unwrap_err().code(), "usage");
        assert_eq!(
            ReadCommand::Status
                .run(&transport, &token(), Some(""))
                .unwrap_err()
                .code(),
            "usage"
        );
        assert_eq!(transport.calls().len(), 0);
    }

    #[test]
    fn locked_vault_gives_exit_four() {
        let transport = FakeTransport::new().reply(423, "{\"error\":\"locked\"}");
        let error = ReadCommand::Catalog(None).run(&transport, &token(), None).unwrap_err();
        assert_eq!(error.exit_code().code(), 4);
        assert_eq!(error.code(), "locked");
    }
}
