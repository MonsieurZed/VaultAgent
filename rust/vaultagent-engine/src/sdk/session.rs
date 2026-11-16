//! Session SDK d'un compte connecté : client, `SyncClient` unique, état des organisations, identifiant et jeton
//! d'accès. Tout l'état du SDK reste en mémoire ; jeter la session purge les clés, les dépôts et les jetons.

use std::sync::Arc;

use bitwarden_auth::token_management::PasswordManagerTokenHandler;
use bitwarden_core::{
    Client, ClientBuilder, ClientSettings, FromClient as _, UserId, auth::JwtToken,
    client::persisted_state::AUTHENTICATION_TOKENS,
};
use bitwarden_pm::migrations::get_sdk_managed_migrations;
use bitwarden_state::{DatabaseConfiguration, registry::StateRegistry};
use bitwarden_sync::{SyncClient, SyncClientExt as _, SyncRequest};
use bitwarden_vault::{Cipher, FolderSyncHandler};
use vaultagent_core::{ErrorCode, Result};

use crate::engine::fail;
use crate::sdk::errors::sync_error;
use crate::sdk::sync_handler::{OrgState, VaultSyncHandler};

/// Nom de l'application annoncé au serveur.
const USER_AGENT: &str = "vaultagent-engine";

/// Client SDK, ses gestionnaires de synchronisation et l'état dérivé d'un compte.
pub struct Session {
    pub(crate) client: Client,
    pub(crate) sync: SyncClient,
    pub(crate) orgs: Arc<OrgState>,
}

impl Session {
    /// Construit un client sans compte : dépôts en mémoire, gestionnaires de dossiers et de fiches enregistrés.
    ///
    /// Paramètres : l'origine du serveur Bitwarden (`https://…`, sans chemin).
    /// Retour : la session, non connectée.
    ///
    /// # Errors
    ///
    /// `engine-init` si l'état du SDK ne s'initialise pas.
    pub async fn build(server: &str) -> Result<Self> {
        let origin = server.trim_end_matches('/').to_owned();
        let settings = ClientSettings {
            identity_url: format!("{origin}/identity"),
            api_url: format!("{origin}/api"),
            user_agent: USER_AGENT.into(),
            ..ClientSettings::default()
        };
        let registry = StateRegistry::new_with_db(DatabaseConfiguration::Memory, get_sdk_managed_migrations())
            .await
            .map_err(|_| fail("engine-init"))?;
        let client = ClientBuilder::new()
            .with_settings(settings)
            .with_token_handler(Arc::new(PasswordManagerTokenHandler::default()))
            .with_state(registry)
            .build();
        let ciphers = client
            .platform()
            .state()
            .get::<Cipher>()
            .map_err(|_| fail("engine-init"))?;
        let orgs = Arc::new(OrgState::default());
        let sync = client.sync();
        sync.register_sync_handler(Arc::new(FolderSyncHandler::from_client(&client)));
        sync.register_sync_handler(Arc::new(VaultSyncHandler::new(client.clone(), ciphers, orgs.clone())));
        Ok(Self { client, sync, orgs })
    }

    /// Déduit l'identifiant d'utilisateur du jeton d'accès et le pose dans le client, sans quoi les dépôts
    /// refusent les lectures (`NotAuthenticatedError`).
    ///
    /// Erreurs : `login:rejected` si le jeton manque ou est illisible.
    pub async fn init_user(&self) -> Result<()> {
        let token = self.access_token().await?;
        let jwt = token.parse::<JwtToken>().map_err(|_| rejected())?;
        let user_id: UserId = jwt.sub.parse().map_err(|_| rejected())?;
        self.client.internal.init_user_id(user_id).await.map_err(|_| rejected())
    }

    /// Jeton d'accès courant (le SDK le renouvelle seul).
    ///
    /// Retour : le jeton ; jamais journalisé.
    ///
    /// # Errors
    ///
    /// `login:rejected` si la session n'a pas de jeton.
    pub async fn access_token(&self) -> Result<String> {
        let tokens = self
            .client
            .platform()
            .state()
            .value::<AUTHENTICATION_TOKENS>()
            .get()
            .await
            .map_err(|_| rejected())?;
        Ok(tokens.access_token)
    }
}

/// Code d'échec d'une session sans jeton valide.
fn rejected() -> ErrorCode {
    crate::engine::stage_error("login", "rejected")
}

impl Session {
    /// Resynchronise le coffre en ignorant le contrôle de date de révision.
    ///
    /// Erreurs : `sync:<catégorie>`.
    pub async fn resync(&self) -> Result<()> {
        self.sync
            .sync(SyncRequest {
                force: true,
                exclude_subdomains: None,
            })
            .await
            .map(|_| ())
            .map_err(|e| sync_error(&e))
    }
}
