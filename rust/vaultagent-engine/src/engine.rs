//! Traits du moteur et de sa fabrique, compte à connecter, fabriques d'erreurs `engine-*` et `<étape>:<catégorie>`.
//!
//! L'interface est bloquante : la façade `async` du moteur natif reste derrière ce trait.

use std::path::Path;

use serde_json::Value;
use vaultagent_core::{ErrorCode, NewItem, Result, RevisionDate, Uuid};
use zeroize::Zeroizing;

use crate::item::Item;
use crate::passkey::{Passkey, PasskeyEntry};
use crate::secret::Secret;
use crate::structure::{Folder, Organization};

/// Compte Bitwarden à connecter.
#[derive(Debug, Clone, Copy)]
pub struct Account<'a> {
    /// Origine du serveur Bitwarden.
    pub server: &'a str,
    /// Courriel du compte.
    pub email: &'a str,
}

/// Moteur d'un profil : une méthode par opération, un échange synchrone par appel.
///
/// Toute méthode échoue avec `engine-locked` si le moteur est arrêté ; les erreurs du serveur Bitwarden arrivent
/// sous la forme `<étape>:<catégorie>` (`login:credentials`, `write:rejected`, `sharing:refused`, …).
pub trait Engine: Send {
    /// Ouvre la session du compte (connexion, ou déverrouillage si le profil est déjà connu).
    ///
    /// Paramètres : le compte, le mot de passe maître et le code de second facteur facultatif.
    ///
    /// # Errors
    ///
    /// `login:*`, `unlock:*`, `account:*` ou un code `engine-*`.
    fn connect(
        &mut self,
        account: &Account<'_>,
        password: &Zeroizing<String>,
        code: Option<&Zeroizing<String>>,
    ) -> Result<()>;

    /// Verrouille le coffre et purge les clés et le contenu déchiffré de la mémoire.
    ///
    /// # Errors
    ///
    /// Code `engine-*` si le moteur est arrêté.
    fn lock(&mut self) -> Result<()>;

    /// Resynchronise le coffre avec le serveur.
    ///
    /// # Errors
    ///
    /// `sync:*` ou un code `engine-*`.
    fn sync(&mut self) -> Result<()>;

    /// Fiches actives, filtrées par la recherche si fournie.
    ///
    /// Paramètres : le texte recherché.
    /// Retour : les fiches hors corbeille.
    ///
    /// # Errors
    ///
    /// `items:*` ou un code `engine-*`.
    fn items(&mut self, search: Option<&str>) -> Result<Vec<Item>>;

    /// Fiches de la corbeille.
    ///
    /// # Errors
    ///
    /// `items:*` ou un code `engine-*`.
    fn trashed_items(&mut self) -> Result<Vec<Item>>;

    /// Lit une fiche, corbeille comprise.
    ///
    /// Paramètres : l'identifiant de la fiche.
    ///
    /// # Errors
    ///
    /// `items:not-found` ou un code `engine-*`.
    fn item(&mut self, id: &Uuid) -> Result<Item>;

    /// Code TOTP courant d'une fiche.
    ///
    /// Paramètres : l'identifiant de la fiche.
    ///
    /// # Errors
    ///
    /// `totp:*` ou un code `engine-*`.
    fn totp(&mut self, id: &Uuid) -> Result<Secret>;

    /// Dossiers personnels.
    ///
    /// # Errors
    ///
    /// `folders:*` ou un code `engine-*`.
    fn folders(&mut self) -> Result<Vec<Folder>>;

    /// Crée un dossier.
    ///
    /// Paramètres : le nom.
    /// Retour : l'identifiant du dossier créé.
    ///
    /// # Errors
    ///
    /// `write:*` ou un code `engine-*`.
    fn create_folder(&mut self, name: &str) -> Result<String>;

    /// Renomme un dossier.
    ///
    /// Paramètres : l'identifiant et le nouveau nom.
    ///
    /// # Errors
    ///
    /// `write:*` ou un code `engine-*`.
    fn rename_folder(&mut self, id: &Uuid, name: &str) -> Result<()>;

    /// Supprime un dossier ; ses fiches restent, sans dossier.
    ///
    /// Paramètres : l'identifiant du dossier.
    ///
    /// # Errors
    ///
    /// `write:*` ou un code `engine-*`.
    fn delete_folder(&mut self, id: &Uuid) -> Result<()>;

    /// Crée une fiche validée.
    ///
    /// Paramètres : la fiche.
    /// Retour : l'identifiant de la fiche créée.
    ///
    /// # Errors
    ///
    /// `write:*` ou un code `engine-*`.
    fn create_item(&mut self, item: &NewItem) -> Result<String>;

    /// Applique un patch d'écriture (voir `vaultagent_core::apply_write_patch`) si la révision attendue est
    /// encore la bonne.
    ///
    /// Paramètres : la fiche, la révision attendue, le patch, l'organisation de la fiche (`None` si personnelle).
    ///
    /// # Errors
    ///
    /// `write:*`, `InvalidPatch`, `RevisionConflict`, `ItemRefused` ou un code `engine-*`.
    fn update_item(
        &mut self,
        id: &Uuid,
        expected_revision: &RevisionDate,
        patch: &Value,
        organization_id: Option<&Uuid>,
    ) -> Result<()>;

    /// Met une fiche à la corbeille.
    ///
    /// Paramètres : l'identifiant de la fiche.
    ///
    /// # Errors
    ///
    /// `write:*` ou un code `engine-*`.
    fn trash_item(&mut self, id: &Uuid) -> Result<()>;

    /// Restaure une fiche de la corbeille.
    ///
    /// Paramètres : l'identifiant de la fiche.
    ///
    /// # Errors
    ///
    /// `write:*` ou un code `engine-*`.
    fn restore_item(&mut self, id: &Uuid) -> Result<()>;

    /// Organisations actives, avec collections et droit d'édition.
    ///
    /// # Errors
    ///
    /// `sharing:*` ou un code `engine-*`.
    fn organizations(&mut self) -> Result<Vec<Organization>>;

    /// Vérifie le droit d'écrire dans chaque collection d'une organisation.
    ///
    /// Paramètres : l'organisation et les collections visées.
    ///
    /// # Errors
    ///
    /// `sharing:refused` si l'une n'est pas éditable, ou un code `engine-*`.
    fn check_sharing(&mut self, organization_id: &Uuid, collection_ids: &[Uuid]) -> Result<()>;

    /// Crée une collection dans une organisation.
    ///
    /// Paramètres : l'organisation et le nom.
    /// Retour : l'identifiant de la collection créée.
    ///
    /// # Errors
    ///
    /// `sharing:*` ou un code `engine-*`.
    fn create_collection(&mut self, organization_id: &Uuid, name: &str) -> Result<String>;

    /// Transfère une fiche personnelle à une organisation, dans des collections.
    ///
    /// Paramètres : la fiche, l'organisation et les collections.
    ///
    /// # Errors
    ///
    /// `sharing:*` ou un code `engine-*`.
    fn share_item(&mut self, id: &Uuid, organization_id: &Uuid, collection_ids: &[Uuid]) -> Result<()>;

    /// Passkeys du coffre, filtrées par partie de confiance si fournie.
    ///
    /// Paramètres : le domaine de la partie de confiance.
    ///
    /// # Errors
    ///
    /// `passkeys:*` ou un code `engine-*`.
    fn passkeys(&mut self, rp_id: Option<&str>) -> Result<Vec<PasskeyEntry>>;

    /// Crée une fiche portant la passkey, la relit et vérifie qu'elle y est.
    ///
    /// Paramètres : l'identifiant FIDO2 complet.
    /// Retour : l'identifiant de la fiche créée.
    ///
    /// # Errors
    ///
    /// `passkeys:passkey-storage` si la relecture diffère, ou un code `engine-*`.
    fn create_passkey(&mut self, credential: &Passkey) -> Result<String>;

    /// Indique si le moteur est utilisable.
    fn alive(&mut self) -> bool;

    /// Arrête le moteur et purge ses secrets ; sans effet s'il est déjà arrêté.
    fn kill(&mut self);
}

/// Fabrique de moteurs, une instance par profil.
pub trait EngineFactory: Send + Sync {
    /// Démarre un moteur non connecté.
    ///
    /// Paramètres : le dossier de données et l'identifiant de profil.
    /// Retour : le moteur démarré.
    ///
    /// # Errors
    ///
    /// `engine-timeout`, `engine-init` ou `engine-stopped` si le démarrage échoue.
    fn start(&self, directory: &Path, profile: &str) -> Result<Box<dyn Engine>>;
}

/// Construit un code d'erreur du moteur.
///
/// Paramètres : le texte du code (`engine-timeout`, `engine-stopped`, …).
/// Retour : `ErrorCode::Engine(code)`.
#[must_use]
pub fn fail(code: &str) -> ErrorCode {
    ErrorCode::Engine(code.to_owned())
}

/// Construit un code `<étape>:<catégorie>` du moteur, comme `login:credentials` ou `write:rejected`.
///
/// Paramètres : l'étape et la catégorie, en minuscules et tirets.
/// Retour : `ErrorCode::Engine("<étape>:<catégorie>")`, ou `RequestRefused` si la forme est invalide.
#[must_use]
pub fn stage_error(stage: &str, category: &str) -> ErrorCode {
    ErrorCode::engine(&format!("{stage}:{category}")).unwrap_or(ErrorCode::RequestRefused)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_error_follows_the_protocol_shape() {
        assert_eq!(stage_error("login", "credentials").to_string(), "login:credentials");
        assert_eq!(stage_error("write", "rejected").to_string(), "write:rejected");
        assert_eq!(stage_error("Bad", "x"), ErrorCode::RequestRefused);
        assert_eq!(fail("engine-timeout").to_string(), "engine-timeout");
    }
}
