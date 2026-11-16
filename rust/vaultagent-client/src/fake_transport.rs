//! Faux serveur en mémoire pour les tests : réponses scriptées et journal des requêtes, sans réseau.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use crate::transport::{Response, Transport, TransportError};

/// Requête reçue par le faux serveur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// Chemin demandé.
    pub path: String,
    /// En-tête `Authorization` envoyé.
    pub bearer: Option<String>,
    /// Corps JSON envoyé.
    pub body: String,
    /// Délai demandé.
    pub timeout: Duration,
}

/// Faux transport : rend les réponses dans l'ordre et note chaque requête.
#[derive(Default)]
pub struct FakeTransport {
    replies: RefCell<VecDeque<Result<Response, TransportError>>>,
    calls: RefCell<Vec<Call>>,
}

impl FakeTransport {
    /// Crée un faux serveur sans réponse.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute une réponse HTTP à rendre à la prochaine requête.
    ///
    /// Paramètres : le statut et le corps texte.
    /// Retour : le faux serveur, pour chaîner.
    #[must_use]
    pub fn reply(self, status: u16, body: &str) -> Self {
        self.replies.borrow_mut().push_back(Ok(Response {
            status,
            body: body.as_bytes().to_vec(),
        }));
        self
    }

    /// Ajoute un échec de transport à rendre à la prochaine requête.
    ///
    /// Paramètres : l'erreur.
    /// Retour : le faux serveur, pour chaîner.
    #[must_use]
    pub fn fail(self, error: TransportError) -> Self {
        self.replies.borrow_mut().push_back(Err(error));
        self
    }

    /// Requêtes reçues jusqu'ici.
    #[must_use]
    pub fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }
}

impl Transport for FakeTransport {
    /// Note la requête puis rend la réponse suivante ; sans réponse prévue, un échec `NotSent`.
    fn post(
        &self,
        path: &str,
        bearer: Option<&str>,
        body: &[u8],
        timeout: Duration,
    ) -> Result<Response, TransportError> {
        self.calls.borrow_mut().push(Call {
            path: path.to_owned(),
            bearer: bearer.map(str::to_owned),
            body: String::from_utf8_lossy(body).into_owned(),
            timeout,
        });
        self.replies
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Err(TransportError::NotSent("No scripted reply.".to_owned())))
    }
}
