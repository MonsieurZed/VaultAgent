//! Lectures du coffre synchronisé : liste des fiches actives et de la corbeille, lecture d'une fiche, code TOTP.
//! Les fiches qui ne se déchiffrent pas sont omises des listes.

use bitwarden_vault::{CipherId, GetCipherError, VaultClientExt as _};
use vaultagent_core::{Result, Uuid};

use crate::engine::stage_error;
use crate::item::Item;
use crate::sdk::convert::item_from_view;
use crate::sdk::filter;
use crate::sdk::session::Session;
use crate::secret::Secret;

/// Identifiant de fiche du SDK.
///
/// Paramètres : l'identifiant validé.
/// Retour : l'identifiant du SDK.
///
/// # Errors
///
/// `items:not-found` si l'identifiant n'a pas la forme d'un UUID du SDK.
pub(super) fn cipher_id(id: &Uuid) -> Result<CipherId> {
    id.as_str().parse().map_err(|_| stage_error("items", "not-found"))
}

/// Code d'une erreur de lecture de fiche.
///
/// Paramètres : l'erreur du SDK.
/// Retour : `items:not-found` si la fiche manque, sinon `items:rejected`.
pub(super) fn read_error(error: &GetCipherError) -> vaultagent_core::ErrorCode {
    match error {
        GetCipherError::ItemNotFound(_) => stage_error("items", "not-found"),
        _ => stage_error("items", "rejected"),
    }
}

/// Toutes les fiches déchiffrées du dépôt local.
///
/// Paramètres : la session.
/// Retour : les fiches, sans celles qui échouent au déchiffrement.
///
/// # Errors
///
/// `items:rejected` si le dépôt est illisible.
async fn all(session: &Session) -> Result<Vec<Item>> {
    let result = session
        .client
        .vault()
        .ciphers()
        .get_all()
        .await
        .map_err(|e| read_error(&e))?;
    Ok(result.successes.iter().map(item_from_view).collect())
}

/// Fiches actives, filtrées par la recherche si elle est fournie.
///
/// Paramètres : la session et le texte cherché.
/// Retour : les fiches de type 1 et 2, hors archive et corbeille.
///
/// # Errors
///
/// `items:rejected`.
pub async fn items(session: &Session, search: Option<&str>) -> Result<Vec<Item>> {
    Ok(filter::active(all(session).await?, search))
}

/// Fiches de la corbeille.
///
/// Paramètres : la session.
/// Retour : les fiches de type 1 et 2 mises à la corbeille.
///
/// # Errors
///
/// `items:rejected`.
pub async fn trashed_items(session: &Session) -> Result<Vec<Item>> {
    Ok(filter::trashed(all(session).await?))
}

/// Lit une fiche, corbeille comprise.
///
/// Paramètres : la session et l'identifiant.
/// Retour : la fiche déchiffrée.
///
/// # Errors
///
/// `items:not-found` si elle n'existe pas ou n'est pas exposée (type autre que 1 et 2, archivée).
pub async fn item(session: &Session, id: &Uuid) -> Result<Item> {
    let view = session
        .client
        .vault()
        .ciphers()
        .get(cipher_id(id)?)
        .await
        .map_err(|e| read_error(&e))?;
    let found = item_from_view(&view);
    if filter::exposed(&found) {
        Ok(found)
    } else {
        Err(stage_error("items", "not-found"))
    }
}

/// Code TOTP courant d'une fiche.
///
/// Paramètres : la session et l'identifiant.
/// Retour : le code à six chiffres (ou celui du format de la graine).
///
/// # Errors
///
/// `items:not-found`, `totp:missing` si la fiche n'a pas de TOTP, `totp:rejected` si la graine est invalide.
pub async fn totp(session: &Session, id: &Uuid) -> Result<Secret> {
    let found = item(session, id).await?;
    let seed = found
        .login
        .and_then(|login| login.totp)
        .filter(|seed| !seed.expose().is_empty())
        .ok_or_else(|| stage_error("totp", "missing"))?;
    session
        .client
        .vault()
        .totp()
        .generate_totp(seed.expose().to_owned(), None)
        .map(|generated| Secret::new(generated.code))
        .map_err(|_| stage_error("totp", "rejected"))
}
