//! Faux système Windows Hello pour les tests : réponses scriptées, journal des appels, aucune invite.

use std::cell::RefCell;

use crate::error::ClientError;
use crate::hello_system::{HelloAssertion, HelloRegistration, HelloStatus, HelloSystem};

/// Comportement scripté de la signature : reçoit l'identifiant et le défi, rend l'assertion.
pub type SignScript = Box<dyn Fn(&[u8], &str) -> Result<HelloAssertion, ClientError>>;

/// Système factice : disponibilité, enregistrement et signature configurables.
pub struct FakeHello {
    available: bool,
    registration: Result<HelloRegistration, ClientError>,
    sign: SignScript,
    registers: RefCell<usize>,
    challenges: RefCell<Vec<String>>,
}

impl FakeHello {
    /// Système disponible avec les réponses données.
    ///
    /// Paramètres : le résultat de l'enregistrement et le comportement de la signature.
    /// Retour : le faux système.
    #[must_use]
    pub fn new(registration: Result<HelloRegistration, ClientError>, sign: SignScript) -> Self {
        Self {
            available: true,
            registration,
            sign,
            registers: RefCell::new(0),
            challenges: RefCell::new(Vec::new()),
        }
    }

    /// Système indisponible : l'enregistrement et la signature refusent.
    ///
    /// Retour : le faux système.
    #[must_use]
    pub fn unavailable() -> Self {
        let mut system = Self::new(
            Err(ClientError::HelloUnavailable),
            Box::new(|_, _| Err(ClientError::HelloUnavailable)),
        );
        system.available = false;
        system
    }

    /// Nombre d'enregistrements demandés jusqu'ici.
    ///
    /// Retour : le compte.
    #[must_use]
    pub fn registers(&self) -> usize {
        *self.registers.borrow()
    }

    /// Défis signés jusqu'ici.
    ///
    /// Retour : la liste, dans l'ordre.
    #[must_use]
    pub fn challenges(&self) -> Vec<String> {
        self.challenges.borrow().clone()
    }
}

impl HelloSystem for FakeHello {
    /// Disponibilité configurée, version 7 si disponible.
    fn status(&self) -> HelloStatus {
        HelloStatus {
            available: self.available,
            api_version: if self.available { 7 } else { 0 },
        }
    }

    /// Compte l'appel puis rend la réponse configurée.
    fn register(&self, _rp: &str, _origin: &str) -> Result<HelloRegistration, ClientError> {
        *self.registers.borrow_mut() += 1;
        self.registration.clone()
    }

    /// Note le défi puis exécute le comportement scripté.
    fn sign(
        &self,
        credential_id: &[u8],
        challenge: &str,
        _rp: &str,
        _origin: &str,
    ) -> Result<HelloAssertion, ClientError> {
        self.challenges.borrow_mut().push(challenge.to_owned());
        (self.sign)(credential_id, challenge)
    }
}
