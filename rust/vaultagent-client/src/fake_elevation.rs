//! Faux système d'élévation pour les tests : jeton configurable, lancement scripté, aucune invite UAC.

use std::cell::RefCell;
use std::fs;
use std::path::Path;

use crate::elevation_system::ElevationSystem;
use crate::error::ClientError;
use crate::protector::{Protector, ReversingProtector};

/// Comportement scripté du lancement élevé : reçoit la ligne d'arguments, rend le code de sortie.
pub type Launch = Box<dyn Fn(&str) -> Result<u32, ClientError>>;

/// Système factice : élevé ou non, avec un SID fixe et un lancement remplaçable.
pub struct FakeElevation {
    elevated: bool,
    sid: String,
    launch: Launch,
    launches: RefCell<Vec<String>>,
}

impl FakeElevation {
    /// Système élevé dont le lancement échoue (rien ne doit être relancé).
    ///
    /// Paramètres : le SID de l'utilisateur factice.
    /// Retour : le faux système.
    #[must_use]
    pub fn elevated(sid: &str) -> Self {
        Self::new(
            true,
            sid,
            Box::new(|_| Err(ClientError::Elevation("No launch scripted.".to_owned()))),
        )
    }

    /// Système non élevé dont le lancement est scripté.
    ///
    /// Paramètres : le SID et le comportement du lancement.
    /// Retour : le faux système.
    #[must_use]
    pub fn standard(sid: &str, launch: Launch) -> Self {
        Self::new(false, sid, launch)
    }

    /// Crée un faux système.
    fn new(elevated: bool, sid: &str, launch: Launch) -> Self {
        Self {
            elevated,
            sid: sid.to_owned(),
            launch,
            launches: RefCell::new(Vec::new()),
        }
    }

    /// Lignes d'arguments des lancements élevés demandés jusqu'ici.
    ///
    /// Retour : la liste, dans l'ordre.
    #[must_use]
    pub fn launches(&self) -> Vec<String> {
        self.launches.borrow().clone()
    }
}

impl ElevationSystem for FakeElevation {
    /// Valeur configurée.
    fn is_elevated(&self) -> bool {
        self.elevated
    }

    /// SID configuré.
    fn current_sid(&self) -> Result<String, ClientError> {
        Ok(self.sid.clone())
    }

    /// Crée un dossier ordinaire : l'ACL n'est pas testée sans élévation.
    fn create_private_directory(&self, path: &Path) -> Result<(), ClientError> {
        fs::create_dir_all(path).map_err(|_| ClientError::Config("Cannot create the test folder.".to_owned()))
    }

    /// Protecteur de test réversible.
    fn machine_protector(&self) -> Result<Box<dyn Protector>, ClientError> {
        Ok(Box::new(ReversingProtector))
    }

    /// Note la ligne puis exécute le comportement scripté.
    fn run_elevated(&self, args: &str) -> Result<u32, ClientError> {
        self.launches.borrow_mut().push(args.to_owned());
        (self.launch)(args)
    }
}
