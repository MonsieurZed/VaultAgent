//! Implémentation du trait `Engine` par le faux moteur : journalise l'appel, puis sert la réponse empilée.

use serde_json::Value;
use vaultagent_core::{NewItem, Result, RevisionDate, Uuid};
use zeroize::Zeroizing;

use crate::engine::{Account, Engine, fail};
use crate::fake::{FakeHandle, locked};
use crate::fake_call::Call;
use crate::fake_reply::Reply;
use crate::item::Item;
use crate::passkey::{Passkey, PasskeyEntry};
use crate::secret::Secret;
use crate::structure::{Folder, Organization};

/// Faux moteur : rend les réponses empilées dans l'ordre, `engine-refused` quand il n'y en a plus.
pub struct FakeEngine {
    handle: FakeHandle,
}

/// Identifiants en texte, pour le journal.
fn texts(ids: &[Uuid]) -> Vec<String> {
    ids.iter().map(|id| id.as_str().to_owned()).collect()
}

impl FakeEngine {
    /// Crée un moteur lié à une poignée de test.
    ///
    /// Paramètres : la poignée.
    /// Retour : le moteur.
    #[must_use]
    pub(crate) fn new(handle: FakeHandle) -> Self {
        Self { handle }
    }

    /// Journalise un appel puis rend la réponse suivante.
    ///
    /// Paramètres : l'appel.
    /// Retour : la réponse empilée.
    ///
    /// # Errors
    ///
    /// `engine-locked` si le moteur est « mort », `engine-refused` s'il n'y a plus de réponse, ou l'échec scripté.
    fn step(&self, call: Call) -> Result<Reply> {
        let mut state = locked(&self.handle.0);
        if !state.alive {
            return Err(fail("engine-locked"));
        }
        state.calls.push(call);
        match state.replies.pop_front() {
            Some(Reply::Failure(code)) => Err(code),
            Some(reply) => Ok(reply),
            None => Err(fail("engine-refused")),
        }
    }
}

impl Engine for FakeEngine {
    /// Journalise l'appel `connect` puis sert la réponse empilée.
    fn connect(
        &mut self,
        account: &Account<'_>,
        password: &Zeroizing<String>,
        code: Option<&Zeroizing<String>>,
    ) -> Result<()> {
        self.step(Call::Connect {
            server: account.server.to_owned(),
            email: account.email.to_owned(),
            password: Secret::from(password.as_str()),
            code: code.map(|text| Secret::from(text.as_str())),
        })?
        .into_unit()
    }

    /// Journalise l'appel `lock` puis sert la réponse empilée.
    fn lock(&mut self) -> Result<()> {
        self.step(Call::Lock)?.into_unit()
    }

    /// Journalise l'appel `sync` puis sert la réponse empilée.
    fn sync(&mut self) -> Result<()> {
        self.step(Call::Sync)?.into_unit()
    }

    /// Journalise l'appel `items` puis sert la réponse empilée.
    fn items(&mut self, search: Option<&str>) -> Result<Vec<Item>> {
        self.step(Call::Items {
            search: search.map(str::to_owned),
        })?
        .into_items()
    }

    /// Journalise l'appel `trashed_items` puis sert la réponse empilée.
    fn trashed_items(&mut self) -> Result<Vec<Item>> {
        self.step(Call::TrashedItems)?.into_items()
    }

    /// Journalise l'appel `item` puis sert la réponse empilée.
    fn item(&mut self, id: &Uuid) -> Result<Item> {
        self.step(Call::Item {
            id: id.as_str().to_owned(),
        })?
        .into_item()
    }

    /// Journalise l'appel `totp` puis sert la réponse empilée.
    fn totp(&mut self, id: &Uuid) -> Result<Secret> {
        self.step(Call::Totp {
            id: id.as_str().to_owned(),
        })?
        .into_secret()
    }

    /// Journalise l'appel `folders` puis sert la réponse empilée.
    fn folders(&mut self) -> Result<Vec<Folder>> {
        self.step(Call::Folders)?.into_folders()
    }

    /// Journalise l'appel `create_folder` puis sert la réponse empilée.
    fn create_folder(&mut self, name: &str) -> Result<String> {
        self.step(Call::CreateFolder { name: name.to_owned() })?.into_text()
    }

    /// Journalise l'appel `rename_folder` puis sert la réponse empilée.
    fn rename_folder(&mut self, id: &Uuid, name: &str) -> Result<()> {
        self.step(Call::RenameFolder {
            id: id.as_str().to_owned(),
            name: name.to_owned(),
        })?
        .into_unit()
    }

    /// Journalise l'appel `delete_folder` puis sert la réponse empilée.
    fn delete_folder(&mut self, id: &Uuid) -> Result<()> {
        self.step(Call::DeleteFolder {
            id: id.as_str().to_owned(),
        })?
        .into_unit()
    }

    /// Journalise l'appel `create_item` puis sert la réponse empilée.
    fn create_item(&mut self, item: &NewItem) -> Result<String> {
        self.step(Call::CreateItem(Box::new(item.clone())))?.into_text()
    }

    /// Journalise l'appel `update_item` puis sert la réponse empilée.
    fn update_item(
        &mut self,
        id: &Uuid,
        expected_revision: &RevisionDate,
        patch: &Value,
        organization_id: Option<&Uuid>,
    ) -> Result<()> {
        self.step(Call::UpdateItem {
            id: id.as_str().to_owned(),
            expected_revision: expected_revision.as_str().to_owned(),
            patch: patch.clone(),
            organization_id: organization_id.map(|org| org.as_str().to_owned()),
        })?
        .into_unit()
    }

    /// Journalise l'appel `trash_item` puis sert la réponse empilée.
    fn trash_item(&mut self, id: &Uuid) -> Result<()> {
        self.step(Call::TrashItem {
            id: id.as_str().to_owned(),
        })?
        .into_unit()
    }

    /// Journalise l'appel `restore_item` puis sert la réponse empilée.
    fn restore_item(&mut self, id: &Uuid) -> Result<()> {
        self.step(Call::RestoreItem {
            id: id.as_str().to_owned(),
        })?
        .into_unit()
    }

    /// Journalise l'appel `organizations` puis sert la réponse empilée.
    fn organizations(&mut self) -> Result<Vec<Organization>> {
        self.step(Call::Organizations)?.into_organizations()
    }

    /// Journalise l'appel `check_sharing` puis sert la réponse empilée.
    fn check_sharing(&mut self, organization_id: &Uuid, collection_ids: &[Uuid]) -> Result<()> {
        self.step(Call::CheckSharing {
            organization_id: organization_id.as_str().to_owned(),
            collection_ids: texts(collection_ids),
        })?
        .into_unit()
    }

    /// Journalise l'appel `create_collection` puis sert la réponse empilée.
    fn create_collection(&mut self, organization_id: &Uuid, name: &str) -> Result<String> {
        self.step(Call::CreateCollection {
            organization_id: organization_id.as_str().to_owned(),
            name: name.to_owned(),
        })?
        .into_text()
    }

    /// Journalise l'appel `share_item` puis sert la réponse empilée.
    fn share_item(&mut self, id: &Uuid, organization_id: &Uuid, collection_ids: &[Uuid]) -> Result<()> {
        self.step(Call::ShareItem {
            id: id.as_str().to_owned(),
            organization_id: organization_id.as_str().to_owned(),
            collection_ids: texts(collection_ids),
        })?
        .into_unit()
    }

    /// Journalise l'appel `passkeys` puis sert la réponse empilée.
    fn passkeys(&mut self, rp_id: Option<&str>) -> Result<Vec<PasskeyEntry>> {
        self.step(Call::Passkeys {
            rp_id: rp_id.map(str::to_owned),
        })?
        .into_passkeys()
    }

    /// Journalise l'appel `create_passkey` puis sert la réponse empilée.
    fn create_passkey(&mut self, credential: &Passkey) -> Result<String> {
        self.step(Call::CreatePasskey(Box::new(credential.clone())))?
            .into_text()
    }

    /// Rend l'état de vie simulé.
    fn alive(&mut self) -> bool {
        locked(&self.handle.0).alive
    }

    /// Marque le moteur comme arrêté et compte l'appel.
    fn kill(&mut self) {
        let mut state = locked(&self.handle.0);
        state.alive = false;
        state.kills += 1;
    }
}
