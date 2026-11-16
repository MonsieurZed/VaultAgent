//! Moteur natif sur le SDK Bitwarden : implémente le trait [`Engine`] derrière un runtime tokio `current_thread`
//! que chaque moteur possède (le trait reste bloquant). Chaque méthode délègue à un module du dossier `sdk`.

use std::fmt;
use std::future::Future;

use serde_json::Value;
use tokio::runtime::{Builder, Runtime};
use vaultagent_core::{ErrorCode, NewItem, Result, RevisionDate, Uuid};
use zeroize::Zeroizing;

use crate::engine::{Account, Engine, fail};
use crate::item::Item;
use crate::passkey::{Passkey, PasskeyEntry};
use crate::sdk::session::Session;
use crate::sdk::{folders, login, org, passkeys, read, share, write};
use crate::secret::Secret;
use crate::structure::{Folder, Organization};

/// Moteur d'un profil : un runtime, et la session du compte tant qu'il est connecté.
pub struct SdkEngine {
    pub(super) runtime: Runtime,
    pub(super) session: Option<Session>,
    alive: bool,
}

impl fmt::Debug for SdkEngine {
    /// N'écrit que l'état (vivant, connecté), jamais de donnée du compte.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SdkEngine")
            .field("alive", &self.alive)
            .field("connected", &self.session.is_some())
            .finish_non_exhaustive()
    }
}

impl SdkEngine {
    /// Crée un moteur non connecté, avec son runtime.
    ///
    /// Retour : le moteur.
    ///
    /// # Errors
    ///
    /// `engine-init` si le runtime ne se crée pas.
    pub fn new() -> Result<Self> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| fail("engine-init"))?;
        Ok(Self {
            runtime,
            session: None,
            alive: true,
        })
    }

    /// Session du compte connecté.
    ///
    /// Retour : la session.
    ///
    /// # Errors
    ///
    /// `engine-locked` si le moteur est arrêté, `locked` s'il n'est pas connecté ou a été verrouillé.
    fn session(&self) -> Result<&Session> {
        if !self.alive {
            return Err(fail("engine-locked"));
        }
        self.session.as_ref().ok_or(ErrorCode::Locked)
    }

    /// Exécute une opération asynchrone sur la session, jusqu'à son terme.
    ///
    /// Paramètres : l'opération, qui reçoit la session.
    /// Retour : son résultat.
    ///
    /// # Errors
    ///
    /// Celles de [`SdkEngine::session`] et de l'opération.
    fn run<'a, T, F, Fut>(&'a self, operation: F) -> Result<T>
    where
        F: FnOnce(&'a Session) -> Fut,
        Fut: Future<Output = Result<T>> + 'a,
    {
        let session = self.session()?;
        self.runtime.block_on(operation(session))
    }

    /// Jette la session : clés, dépôts et jetons sont libérés dans le contexte du runtime.
    fn drop_session(&mut self) {
        let old = self.session.take();
        self.runtime.block_on(async move { drop(old) });
    }
}

impl Engine for SdkEngine {
    /// Ouvre une session neuve, l'ancienne étant jetée d'abord ; un échec laisse le moteur non connecté.
    fn connect(
        &mut self,
        account: &Account<'_>,
        password: &Zeroizing<String>,
        code: Option<&Zeroizing<String>>,
    ) -> Result<()> {
        if !self.alive {
            return Err(fail("engine-locked"));
        }
        self.drop_session();
        let session = self.runtime.block_on(login::connect(account, password, code))?;
        self.session = Some(session);
        Ok(())
    }

    /// Jette le client SDK : plus aucune clé, fiche ni jeton en mémoire ; les lectures rendent `locked`.
    fn lock(&mut self) -> Result<()> {
        if !self.alive {
            return Err(fail("engine-locked"));
        }
        self.drop_session();
        Ok(())
    }

    /// Resynchronise le coffre avec le serveur.
    fn sync(&mut self) -> Result<()> {
        self.run(Session::resync)
    }

    /// Fiches actives, filtrées par la recherche.
    fn items(&mut self, search: Option<&str>) -> Result<Vec<Item>> {
        self.run(|session| read::items(session, search))
    }

    /// Fiches de la corbeille.
    fn trashed_items(&mut self) -> Result<Vec<Item>> {
        self.run(read::trashed_items)
    }

    /// Lit une fiche.
    fn item(&mut self, id: &Uuid) -> Result<Item> {
        self.run(|session| read::item(session, id))
    }

    /// Code TOTP courant d'une fiche.
    fn totp(&mut self, id: &Uuid) -> Result<Secret> {
        self.run(|session| read::totp(session, id))
    }

    /// Dossiers personnels.
    fn folders(&mut self) -> Result<Vec<Folder>> {
        self.run(folders::folders)
    }

    /// Crée un dossier.
    fn create_folder(&mut self, name: &str) -> Result<String> {
        self.run(|session| folders::create_folder(session, name))
    }

    /// Renomme un dossier.
    fn rename_folder(&mut self, id: &Uuid, name: &str) -> Result<()> {
        self.run(|session| folders::rename_folder(session, id, name))
    }

    /// Supprime un dossier.
    fn delete_folder(&mut self, id: &Uuid) -> Result<()> {
        self.run(|session| folders::delete_folder(session, id))
    }

    /// Crée une fiche.
    fn create_item(&mut self, item: &NewItem) -> Result<String> {
        self.run(|session| write::create_item(session, item))
    }

    /// Met à jour une fiche si sa révision est celle attendue.
    fn update_item(
        &mut self,
        id: &Uuid,
        expected_revision: &RevisionDate,
        patch: &Value,
        organization_id: Option<&Uuid>,
    ) -> Result<()> {
        self.run(|session| write::update_item(session, id, expected_revision, patch, organization_id))
    }

    /// Met une fiche à la corbeille.
    fn trash_item(&mut self, id: &Uuid) -> Result<()> {
        self.run(|session| write::trash_item(session, id))
    }

    /// Restaure une fiche de la corbeille.
    fn restore_item(&mut self, id: &Uuid) -> Result<()> {
        self.run(|session| write::restore_item(session, id))
    }

    /// Organisations actives avec leurs collections.
    fn organizations(&mut self) -> Result<Vec<Organization>> {
        self.session().map(org::organizations)
    }

    /// Vérifie le droit d'écrire dans les collections visées.
    fn check_sharing(&mut self, organization_id: &Uuid, collection_ids: &[Uuid]) -> Result<()> {
        self.session()
            .and_then(|session| org::check_sharing(session, organization_id, collection_ids))
    }

    /// Crée une collection dans une organisation.
    fn create_collection(&mut self, organization_id: &Uuid, name: &str) -> Result<String> {
        self.run(|session| share::create_collection(session, organization_id, name))
    }

    /// Transfère une fiche personnelle à une organisation.
    fn share_item(&mut self, id: &Uuid, organization_id: &Uuid, collection_ids: &[Uuid]) -> Result<()> {
        self.run(|session| share::share_item(session, id, organization_id, collection_ids))
    }

    /// Passkeys des fiches actives.
    fn passkeys(&mut self, rp_id: Option<&str>) -> Result<Vec<PasskeyEntry>> {
        self.run(|session| passkeys::list(session, rp_id))
    }

    /// Crée la fiche d'une passkey et vérifie son stockage.
    fn create_passkey(&mut self, credential: &Passkey) -> Result<String> {
        self.run(|session| passkeys::create(session, credential))
    }

    /// Vrai tant que le moteur n'a pas été arrêté.
    fn alive(&mut self) -> bool {
        self.alive
    }

    /// Arrête le moteur et jette la session ; sans effet s'il est déjà arrêté.
    fn kill(&mut self) {
        if self.alive {
            self.alive = false;
            self.drop_session();
        }
    }
}
