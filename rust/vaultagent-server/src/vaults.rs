//! Coffres : un magasin et une session par coffre (`vaults/<id>/`, le coffre `default` à la racine), registre
//! `vaults.json` validé par `vaultagent-core`.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use vaultagent_core::vault_clients::{AccountRef, ensure_unique_account};
use vaultagent_core::vault_registry::{DEFAULT_ID, VaultEntry};
use vaultagent_core::{Devices, UnlockRequests, VaultMetadata, VaultRegistry};
use vaultagent_engine::EngineFactory;

use crate::clock::Clock;
use crate::configure_input::ConfigureInput;
use crate::error::{Result, ServerError};
use crate::session::{Session, SessionStatus};
use crate::shared_entropy::SharedEntropy;
use crate::store::Store;
use crate::sync::locked;

const REGISTRY_FILE: &str = "vaults.json";
const VAULTS_DIRECTORY: &str = "vaults";

/// Un coffre : son magasin, sa session et l'état en mémoire des associations et des demandes.
pub struct VaultContext {
    /// `default` ou UUID.
    pub id: String,
    name: Mutex<String>,
    /// Magasin du coffre.
    pub store: Arc<Store>,
    /// Session du coffre.
    pub session: Arc<Session>,
    /// Codes d'association en attente.
    pub devices: Mutex<Devices>,
    /// Demandes de déverrouillage.
    pub approvals: Mutex<UnlockRequests>,
}

impl VaultContext {
    /// Nom affiché du coffre.
    ///
    /// Paramètres : aucun.
    /// Retour : le nom.
    #[must_use]
    pub fn name(&self) -> String {
        locked(&self.name).clone()
    }
}

/// Ensemble des coffres du serveur.
pub struct Vaults {
    root: Arc<Store>,
    factory: Arc<dyn EngineFactory>,
    clock: Arc<dyn Clock>,
    entropy: SharedEntropy,
    registry: Mutex<VaultRegistry>,
    contexts: Mutex<Vec<Arc<VaultContext>>>,
    tail: Mutex<()>,
}

impl Vaults {
    /// Charge `vaults.json` et ouvre le magasin de chaque coffre.
    ///
    /// Paramètres : le magasin racine (coffre `default`), la fabrique de moteurs, l'horloge, la source d'aléa.
    /// Retour : les coffres.
    ///
    /// # Errors
    ///
    /// `invalid-vault-registry`, `invalid-vault`, ou une erreur du magasin d'un coffre.
    pub fn open(
        root: Arc<Store>,
        factory: Arc<dyn EngineFactory>,
        clock: Arc<dyn Clock>,
        entropy: SharedEntropy,
    ) -> Result<Self> {
        let raw: Option<Value> = root.load_optional(REGISTRY_FILE)?;
        let registry = VaultRegistry::init(raw.as_ref())?;
        let vaults = Self {
            root,
            factory,
            clock,
            entropy,
            registry: Mutex::new(registry.clone()),
            contexts: Mutex::new(Vec::new()),
            tail: Mutex::new(()),
        };
        for entry in registry.entries() {
            let context = vaults.attach(entry)?;
            locked(&vaults.contexts).push(context);
        }
        Ok(vaults)
    }

    /// Construit le contexte d'un coffre, en ouvrant son magasin.
    ///
    /// Paramètres : l'entrée du registre.
    /// Retour : le contexte.
    ///
    /// # Errors
    ///
    /// Erreur du magasin du coffre.
    fn attach(&self, entry: &VaultEntry) -> Result<Arc<VaultContext>> {
        let store = if entry.id == DEFAULT_ID {
            Arc::clone(&self.root)
        } else {
            Arc::new(Store::open(
                &self.root.directory().join(VAULTS_DIRECTORY).join(&entry.id),
            )?)
        };
        let session = Session::new(
            Arc::clone(&store),
            Arc::clone(&self.factory),
            Arc::clone(&self.clock),
            self.entropy.clone(),
        );
        Ok(Arc::new(VaultContext {
            id: entry.id.clone(),
            name: Mutex::new(entry.name.clone()),
            store,
            session: Arc::new(session),
            devices: Mutex::new(Devices::new()),
            approvals: Mutex::new(UnlockRequests::new(&entry.id)),
        }))
    }

    /// Coffres, dans l'ordre du registre.
    ///
    /// Paramètres : aucun.
    /// Retour : les contextes.
    #[must_use]
    pub fn list(&self) -> Vec<Arc<VaultContext>> {
        locked(&self.contexts).clone()
    }

    /// Cherche un coffre.
    ///
    /// Paramètres : l'identifiant, `default` s'il est absent.
    /// Retour : le contexte.
    ///
    /// # Errors
    ///
    /// `vault-missing` si l'identifiant est inconnu.
    pub fn get(&self, id: Option<&str>) -> Result<Arc<VaultContext>> {
        let wanted = id.unwrap_or(DEFAULT_ID);
        self.list()
            .into_iter()
            .find(|context| context.id == wanted)
            .ok_or_else(|| vaultagent_core::ErrorCode::VaultMissing.into())
    }

    /// Identifiant, nom et statut de chaque coffre.
    ///
    /// Paramètres : aucun.
    /// Retour : la liste de `/api/agent/vaults`, tous coffres confondus.
    #[must_use]
    pub fn metadata(&self) -> Vec<VaultMetadata<SessionStatus>> {
        self.list().iter().map(|context| Self::metadata_of(context)).collect()
    }

    /// Identifiant, nom et statut d'un coffre.
    ///
    /// Paramètres : le coffre.
    /// Retour : l'entrée de métadonnées.
    #[must_use]
    pub fn metadata_of(context: &VaultContext) -> VaultMetadata<SessionStatus> {
        VaultMetadata {
            id: context.id.clone(),
            name: context.name(),
            status: context.session.status(),
        }
    }

    /// Ajoute un coffre (sans identifiant) ou renomme un coffre existant, puis écrit `vaults.json`.
    ///
    /// Paramètres : l'identifiant à renommer, le nom.
    /// Retour : les métadonnées du coffre.
    ///
    /// # Errors
    ///
    /// `invalid-vault-name`, `vault-limit`, `vault-missing`, ou une erreur d'écriture ; l'état est alors inchangé.
    pub fn save_name(&self, id: Option<&str>, name: Option<&str>) -> Result<VaultMetadata<SessionStatus>> {
        let _tail = locked(&self.tail);
        let saved = locked(&self.registry).save_name(id, name, &mut self.entropy.clone())?;
        let context = if saved.created {
            self.attach(&saved.entry)?
        } else {
            self.get(id)?
        };
        let previous = context.name();
        locked(&context.name).clone_from(&saved.entry.name);
        if let Err(error) = self.root.save(REGISTRY_FILE, &saved.registry.to_json()) {
            *locked(&context.name) = previous;
            return Err(error);
        }
        *locked(&self.registry) = saved.registry;
        if saved.created {
            locked(&self.contexts).push(Arc::clone(&context));
        }
        Ok(Self::metadata_of(&context))
    }

    /// Configure le compte d'un coffre après avoir refusé un compte déjà présent dans un autre coffre.
    ///
    /// Paramètres : le coffre, l'entrée de `configure`.
    /// Retour : le statut de la session du coffre.
    ///
    /// # Errors
    ///
    /// `duplicate-account`, ou les erreurs de [`Session::configure`].
    pub fn configure(&self, context: &VaultContext, input: &ConfigureInput) -> Result<SessionStatus> {
        let _tail = locked(&self.tail);
        let others: Vec<_> = self
            .list()
            .iter()
            .filter(|other| other.id != context.id)
            .filter_map(|other| other.store.account())
            .collect();
        let references: Vec<AccountRef<'_>> = others
            .iter()
            .map(|account| AccountRef {
                server: &account.server,
                email: &account.email,
            })
            .collect();
        ensure_unique_account(
            &references,
            &AccountRef {
                server: &input.server,
                email: &input.email,
            },
        )
        .map_err(ServerError::from)?;
        context.session.configure(input)
    }

    /// Verrouille les sessions dont le délai est échu ou dont le moteur est mort.
    ///
    /// Paramètres : aucun.
    pub fn expire(&self) {
        self.list().iter().for_each(|context| context.session.expire());
    }

    /// Verrouille toutes les sessions.
    ///
    /// Paramètres : aucun.
    pub fn lock(&self) {
        self.list().iter().for_each(|context| context.session.lock());
    }
}
