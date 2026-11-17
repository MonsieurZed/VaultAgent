//! Travaux envoyés au fil réseau de l'application de barre et réponses rendues à la boucle de messages.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};

use vaultagent_win32::Poster;

use crate::error::ClientError;
use crate::tray_actions::Decision;
use crate::tray_select::ValidationMode;
use crate::tray_status::{TrayRequest, VaultSnapshot};

/// Code posté à la fenêtre : des réponses attendent dans la boîte.
pub const CODE_REPLY: usize = 1;

/// Code posté à la fenêtre : la carte d'approbation est fermée.
pub const CODE_CARD_CLOSED: usize = 2;

/// Code posté à la fenêtre : la fenêtre d'association est fermée.
pub const CODE_PAIR_CLOSED: usize = 3;

/// Travail demandé au fil réseau.
#[derive(Debug, Clone)]
pub enum Job {
    /// Relit l'état de tous les coffres.
    Refresh,
    /// Décide une demande : en mode UAC en une étape, en mode carte selon `approve`.
    Decide {
        /// La demande.
        request: Box<TrayRequest>,
        /// Le mode de son coffre.
        mode: ValidationMode,
        /// Vrai pour approuver (ignoré en mode UAC, qui approuve puis refuse si l'invite est annulée).
        approve: bool,
        /// Vrai pour toujours accepter ce client.
        remember: bool,
    },
    /// Verrouille tous les coffres.
    Lock,
    /// Désactive l'acceptation automatique.
    ResetAutomatic,
    /// Bascule la validation vers un mode.
    Switch(ValidationMode),
    /// Active Windows Hello sur les coffres autorisés.
    EnrollHello,
    /// Demande l'association de ce client.
    Pair {
        /// Adresse HTTPS saisie.
        server: String,
        /// Vrai pour enregistrer ce poste comme approbateur.
        approver: bool,
    },
    /// Arrête le fil.
    Stop,
}

/// Réponse du fil réseau.
#[derive(Debug)]
pub enum Reply {
    /// État des coffres avec l'adresse du serveur.
    Snapshots {
        /// Adresse du serveur associé.
        server: String,
        /// État de chaque coffre.
        vaults: Vec<VaultSnapshot>,
    },
    /// Aucune association.
    NotPaired,
    /// Serveur injoignable ou refus.
    Unavailable,
    /// Décision envoyée ou échouée.
    Decided {
        /// Clé de notification de la demande.
        key: String,
        /// Issue de la décision.
        result: Result<Decision, ClientError>,
    },
    /// Bulle d'information déjà traduite.
    Notice {
        /// Texte.
        text: String,
        /// Vrai pour un avertissement.
        warning: bool,
    },
    /// Code de contrôle de l'association en cours.
    Verification(String),
    /// Fin de l'association : texte traduit pour la fenêtre et réussite.
    Paired {
        /// Texte pour la fenêtre.
        text: String,
        /// Vrai si le client est associé.
        ok: bool,
    },
}

/// Boîte des réponses partagée entre le fil réseau et la boucle de messages.
pub type Mailbox = Arc<Mutex<VecDeque<Reply>>>;

/// Dépose une réponse et réveille la fenêtre.
///
/// Paramètres : la boîte, la fenêtre à réveiller et la réponse.
/// Retour : aucun ; une boîte empoisonnée ou une fenêtre fermée ignore la réponse.
pub fn deliver(mailbox: &Mailbox, poster: Poster, reply: Reply) {
    if let Ok(mut queue) = mailbox.lock() {
        queue.push_back(reply);
    }
    poster.post(CODE_REPLY);
}

/// Vide la boîte.
///
/// Paramètres : la boîte.
/// Retour : les réponses en attente, dans l'ordre d'arrivée.
#[must_use]
pub fn drain(mailbox: &Mailbox) -> Vec<Reply> {
    mailbox
        .lock()
        .map_or_else(|_| Vec::new(), |mut queue| queue.drain(..).collect())
}

/// Sortie de l'association qui relève le code de vérification et le dépose dans la boîte.
pub struct NoticeSink {
    mailbox: Mailbox,
    poster: Poster,
}

impl NoticeSink {
    /// Crée la sortie.
    ///
    /// Paramètres : la boîte et la fenêtre à réveiller.
    /// Retour : la sortie.
    #[must_use]
    pub fn new(mailbox: Mailbox, poster: Poster) -> Self {
        Self { mailbox, poster }
    }
}

/// Extrait le code de vérification d'un texte de l'association.
///
/// Paramètres : le texte écrit par `pair_request`.
/// Retour : le code de la ligne `Verification code: ...`, s'il y en a une.
#[must_use]
pub fn verification_code(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("Verification code: "))
        .map(|code| code.trim().to_owned())
        .filter(|code| !code.is_empty())
}

impl Write for NoticeSink {
    /// Relève le code de vérification et ignore le reste du texte.
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if let Some(code) = verification_code(&String::from_utf8_lossy(buffer)) {
            deliver(&self.mailbox, self.poster, Reply::Verification(code));
        }
        Ok(buffer.len())
    }

    /// Rien à vider.
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_is_read_from_the_announcement() {
        let text = "Verification code: ABCD-EFGH\nApprove this request on https://x (Clients tab).";
        assert_eq!(verification_code(text).as_deref(), Some("ABCD-EFGH"));
        assert_eq!(verification_code("Approve this request"), None);
        assert_eq!(verification_code("Verification code:  \n"), None);
    }
}
