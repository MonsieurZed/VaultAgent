//! Transport HTTP du client : trait, réponse, erreurs et délais du protocole.

use std::time::Duration;

/// Taille maximale d'une réponse acceptée (4 Mio).
pub const MAX_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;

/// Délai d'établissement de la connexion.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Délai d'une requête de lecture ou d'écriture.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(110);

/// Délai d'une requête d'association.
pub const PAIRING_TIMEOUT: Duration = Duration::from_secs(15);

/// Réponse brute du serveur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// Statut HTTP.
    pub status: u16,
    /// Corps, de 4 Mio au plus.
    pub body: Vec<u8>,
}

/// Échec de transport, classé selon que la requête est partie ou non.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// Rien n'a été envoyé : résolution, connexion ou certificat refusés.
    NotSent(String),
    /// Délai ou coupure après l'envoi : le serveur a peut-être agi.
    Indeterminate,
    /// Réponse de plus de 4 Mio.
    TooLarge,
    /// Le serveur a demandé une redirection, refusée.
    Redirected,
}

/// Envoi d'une requête `POST` JSON au serveur ; les tests le remplacent par un faux serveur en mémoire.
pub trait Transport {
    /// Envoie un `POST` et lit la réponse.
    ///
    /// Paramètres : le chemin (`/api/agent/status`…), la valeur de l'en-tête `Authorization` s'il y en a une, le
    /// corps JSON, le délai total de la requête.
    /// Retour : le statut et le corps, quel que soit le statut HTTP ; une erreur seulement si l'échange échoue.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    fn post(
        &self,
        path: &str,
        bearer: Option<&str>,
        body: &[u8],
        timeout: Duration,
    ) -> Result<Response, TransportError>;
}
