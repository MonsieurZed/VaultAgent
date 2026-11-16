//! Configuration et déverrouillage de la session : connexion du moteur, enregistrement du compte, bascule.

use std::sync::Arc;

use vaultagent_core::device_token::random_uuid;
use vaultagent_engine::{Account as EngineAccount, Engine};
use zeroize::Zeroizing;

use crate::account::Account;
use crate::configure_input::{ConfigureInput, usable_password};
use crate::engine_slot::EngineSlot;
use crate::error::{Result, ServerError};
use crate::session::{Session, SessionStatus};
use crate::sync::locked;

impl Session {
    /// Configure le compte : valide l'entrée, connecte un moteur candidat, enregistre `account.json` puis bascule.
    /// Un autre serveur ou un autre courriel crée un nouveau profil et efface les appareils.
    ///
    /// Paramètres : l'entrée de `configure`.
    /// Retour : le statut, session déverrouillée.
    ///
    /// # Errors
    ///
    /// `invalid-account`, `invalid-timeout`, `password-required`, `cancelled` (verrouillage pendant la
    /// connexion), toute erreur du moteur, du magasin ou du chiffrement.
    pub fn configure(&self, input: &ConfigureInput) -> Result<SessionStatus> {
        let _serial = self.serial();
        input.check_account()?;
        let timeout = input.timeout()?;
        let previous = self.store.account();
        let email = input.email.to_lowercase();
        let changed = previous
            .as_ref()
            .is_none_or(|old| old.server != input.server || old.email.to_lowercase() != email);
        let password = self.password_for(input, previous.as_ref(), changed)?;
        let profile = match &previous {
            Some(old) if !changed => old.profile.clone(),
            _ => random_uuid(&mut self.entropy.clone())?,
        };
        let account = Account {
            server: input.server.clone(),
            email,
            timeout_minutes: timeout,
            profile,
            password: self.store.protect(&password, &mut self.entropy.clone())?,
        };
        let generation = self.generation();
        let mut candidate = self.connect(&account, &password, input.code.as_ref())?;
        if generation != self.generation() {
            candidate.kill();
            return Err(ServerError::Cancelled);
        }
        if let Err(error) = self.persist(&account, changed) {
            candidate.kill();
            return Err(error);
        }
        let mut state = locked(&self.state);
        Self::lock_in(&mut state);
        self.store.set_account(Some(account));
        state.engine = Some(Arc::new(EngineSlot::new(candidate)));
        self.touch_in(&mut state);
        drop(state);
        Ok(self.status())
    }

    /// Déverrouille avec le compte enregistré ; sans effet (hors prolongation) si la session est déjà ouverte.
    ///
    /// Paramètres : le code de double authentification facultatif.
    /// Retour : le statut.
    ///
    /// # Errors
    ///
    /// `not-configured`, `cancelled`, `invalid-key` (mot de passe indéchiffrable), toute erreur du moteur.
    pub fn unlock(&self, code: Option<&Zeroizing<String>>) -> Result<SessionStatus> {
        let _serial = self.serial();
        let account = self.store.account().ok_or(vaultagent_core::ErrorCode::NotConfigured)?;
        self.expire();
        if locked(&self.state).engine.is_some() {
            self.touch();
            return Ok(self.status());
        }
        let generation = self.generation();
        let password = self.store.unprotect(&account.password)?;
        let mut candidate = self.connect(&account, &password, code)?;
        let mut state = locked(&self.state);
        if generation != state.generation {
            drop(state);
            candidate.kill();
            return Err(ServerError::Cancelled);
        }
        state.engine = Some(Arc::new(EngineSlot::new(candidate)));
        self.touch_in(&mut state);
        drop(state);
        Ok(self.status())
    }

    /// Choisit le mot de passe : celui de l'entrée, sinon celui du compte enregistré si le compte ne change pas.
    ///
    /// Paramètres : l'entrée, le compte précédent, vrai si le serveur ou le courriel change.
    /// Retour : le mot de passe en clair.
    ///
    /// # Errors
    ///
    /// `password-required` s'il n'y en a pas ou s'il est trop long ; `invalid-key` si l'ancien est indéchiffrable.
    fn password_for(
        &self,
        input: &ConfigureInput,
        previous: Option<&Account>,
        changed: bool,
    ) -> Result<Zeroizing<String>> {
        let given = input.password.as_ref().filter(|value| !value.is_empty());
        let password = match (given, previous) {
            (Some(value), _) => value.clone(),
            (None, Some(old)) if !changed => self.store.unprotect(&old.password)?,
            _ => return Err(ServerError::PasswordRequired),
        };
        if usable_password(&password) {
            Ok(password)
        } else {
            Err(ServerError::PasswordRequired)
        }
    }

    /// Enregistre le compte, après avoir effacé les appareils si le compte a changé.
    ///
    /// Paramètres : le compte, vrai s'il a changé.
    /// Retour : rien.
    ///
    /// # Errors
    ///
    /// Erreur d'écriture de `devices.json` ou `account.json`.
    fn persist(&self, account: &Account, changed: bool) -> Result<()> {
        if changed {
            self.store.update_devices(|_| Ok(Vec::new()))?;
        }
        self.store.save("account.json", account)
    }

    /// Démarre un moteur et ouvre la session du compte ; le moteur est arrêté si la connexion échoue.
    ///
    /// Paramètres : le compte, le mot de passe en clair, le code facultatif.
    /// Retour : le moteur connecté.
    ///
    /// # Errors
    ///
    /// Erreur de démarrage de la fabrique (`engine-init`, `engine-timeout`, …) ou de connexion du moteur
    /// (`login:*`, `unlock:*`, `engine-*`).
    fn connect(
        &self,
        account: &Account,
        password: &Zeroizing<String>,
        code: Option<&Zeroizing<String>>,
    ) -> Result<Box<dyn Engine>> {
        let target = EngineAccount {
            server: &account.server,
            email: &account.email,
        };
        let mut engine = self.factory.start(self.store.directory(), &account.profile)?;
        match engine.connect(&target, password, code) {
            Ok(()) => Ok(engine),
            Err(error) => {
                engine.kill();
                Err(error.into())
            }
        }
    }
}
