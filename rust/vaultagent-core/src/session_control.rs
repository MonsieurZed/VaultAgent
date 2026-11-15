//! Abstraction de la session du coffre utilisée par les demandes de déverrouillage : le moteur réel (tâche 2.x)
//! et les tests fournissent chacun leur implémentation.

use crate::error::Result;

/// Session du coffre telle que la voient les demandes de déverrouillage.
pub trait SessionControl {
    /// Indique si le coffre est déverrouillé ; peut verrouiller d'abord si le délai est échu (équivalent de
    /// `session.status()`).
    ///
    /// Paramètres : aucun.
    /// Retour : `true` si le coffre est ouvert.
    fn unlocked(&mut self) -> bool;

    /// Génération de la session, incrémentée à chaque verrouillage.
    ///
    /// Paramètres : aucun.
    /// Retour : le compteur.
    fn generation(&self) -> u64;

    /// Profil du compte configuré.
    ///
    /// Paramètres : aucun.
    /// Retour : le profil, absent si le coffre n'est pas configuré.
    fn profile(&self) -> Option<String>;

    /// Déverrouille le coffre.
    ///
    /// Paramètres : le code de double authentification, si l'appelant en fournit un.
    /// Retour : rien.
    /// # Errors
    ///
    /// Le code d'erreur du moteur si l'ouverture échoue.
    fn unlock(&mut self, code: Option<&str>) -> Result<()>;

    /// Verrouille le coffre et incrémente la génération.
    ///
    /// Paramètres : aucun.
    /// Retour : rien.
    fn lock(&mut self);
}
