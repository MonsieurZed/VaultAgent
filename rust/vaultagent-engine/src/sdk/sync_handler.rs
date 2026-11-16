//! Gestionnaire de synchronisation que le SDK n'apporte pas : dépose les fiches dans le dépôt en mémoire, retient
//! les organisations actives et leurs collections, et charge les clés d'organisation (sans elles, une fiche
//! partagée ne se déchiffre pas).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use bitwarden_api_api::models::{OrganizationUserStatusType, OrganizationUserType, PolicyType, SyncResponseModel};
use bitwarden_collections::collection::Collection;
use bitwarden_core::{Client, OrganizationId, key_management::crypto::InitOrgCryptoRequest};
use bitwarden_crypto::UnsignedSharedKey;
use bitwarden_state::repository::Repository;
use bitwarden_sync::{SyncHandler, SyncHandlerError};
use bitwarden_vault::Cipher;

/// Organisation active de l'utilisateur, telle que la synchronisation la décrit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgRecord {
    /// Identifiant de l'organisation.
    pub id: String,
    /// Nom de l'organisation.
    pub name: String,
    /// Rôle de l'utilisateur (type d'organisation Bitwarden).
    pub role: i64,
}

/// Organisations et collections reçues à la dernière synchronisation.
#[derive(Default)]
pub struct OrgState {
    organizations: Mutex<Vec<OrgRecord>>,
    collections: Mutex<Vec<Collection>>,
    data_ownership: AtomicBool,
}

impl OrgState {
    /// Indique si une politique `OrganizationDataOwnership` active s'applique à l'utilisateur.
    ///
    /// Retour : vrai si la dernière synchronisation en portait une ; les fiches personnelles sont alors refusées.
    pub fn data_ownership(&self) -> bool {
        self.data_ownership.load(Ordering::Relaxed)
    }

    /// Verrouille un champ en ignorant l'empoisonnement (les données restent cohérentes : remplacement en bloc).
    fn guard<T>(field: &Mutex<T>) -> MutexGuard<'_, T> {
        field.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Organisations actives.
    ///
    /// Retour : une copie de la liste.
    pub fn organizations(&self) -> Vec<OrgRecord> {
        Self::guard(&self.organizations).clone()
    }

    /// Collections chiffrées de toutes les organisations.
    ///
    /// Retour : une copie de la liste.
    pub fn collections(&self) -> Vec<Collection> {
        Self::guard(&self.collections).clone()
    }

    /// Remplace les organisations et collections retenues.
    ///
    /// Paramètres : les nouvelles listes.
    fn replace(&self, organizations: Vec<OrgRecord>, collections: Vec<Collection>) {
        *Self::guard(&self.organizations) = organizations;
        *Self::guard(&self.collections) = collections;
    }
}

/// Numéro de rôle d'organisation dans le protocole (0 propriétaire, 1 administrateur, 2 utilisateur, 4 personnalisé).
///
/// Paramètres : le type d'utilisateur d'organisation renvoyé par le serveur.
/// Retour : son numéro.
#[must_use]
pub fn role_number(role: &OrganizationUserType) -> i64 {
    match role {
        OrganizationUserType::Owner => 0,
        OrganizationUserType::Admin => 1,
        OrganizationUserType::User => 2,
        OrganizationUserType::Custom => 4,
        OrganizationUserType::__Unknown(value) => *value,
    }
}

/// Gestionnaire enregistré sur l'unique `SyncClient` du moteur.
pub struct VaultSyncHandler {
    client: Client,
    ciphers: Arc<dyn Repository<Cipher>>,
    state: Arc<OrgState>,
}

impl VaultSyncHandler {
    /// Crée le gestionnaire.
    ///
    /// Paramètres : le client SDK, le dépôt des fiches et l'état des organisations.
    /// Retour : le gestionnaire.
    #[must_use]
    pub fn new(client: Client, ciphers: Arc<dyn Repository<Cipher>>, state: Arc<OrgState>) -> Self {
        Self { client, ciphers, state }
    }
}

#[async_trait]
impl SyncHandler for VaultSyncHandler {
    /// Remplace les fiches, retient les organisations actives, charge leurs clés puis range les collections.
    async fn on_sync(&self, response: &SyncResponseModel) -> Result<(), SyncHandlerError> {
        let mut items = Vec::new();
        for model in response.ciphers.clone().unwrap_or_default() {
            let cipher = Cipher::try_from(model)?;
            let id = cipher.id.ok_or("fiche sans identifiant")?;
            items.push((id, cipher));
        }
        self.ciphers.replace_all(items).await?;

        let mut keys = HashMap::new();
        let mut organizations = Vec::new();
        let profile = response.profile.as_ref().and_then(|p| p.organizations.clone());
        for org in profile.unwrap_or_default() {
            let active = org.enabled.unwrap_or(false) && org.status == Some(OrganizationUserStatusType::Confirmed);
            if let (true, Some(id), Some(key)) = (active, org.id, org.key) {
                let role = org.r#type.as_ref().map_or(2, role_number);
                organizations.push(OrgRecord {
                    id: id.to_string(),
                    name: org.name.unwrap_or_default(),
                    role,
                });
                keys.insert(
                    id.to_string().parse::<OrganizationId>()?,
                    key.parse::<UnsignedSharedKey>()?,
                );
            }
        }
        self.client
            .crypto()
            .initialize_org_crypto(InitOrgCryptoRequest {
                organization_keys: keys,
            })
            .await?;

        let mut collections = Vec::new();
        for model in response.collections.clone().unwrap_or_default() {
            collections.push(Collection::try_from(model)?);
        }
        let owned =
            response.policies.iter().flatten().any(|policy| {
                policy.enabled == Some(true) && policy.r#type == Some(PolicyType::OrganizationDataOwnership)
            });
        self.state.data_ownership.store(owned, Ordering::Relaxed);
        self.state.replace(organizations, collections);
        Ok(())
    }
}
