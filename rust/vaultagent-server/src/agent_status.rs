//! Routes agent d'état : `vaults`, `status` et `lock`.

use serde::Serialize;
use vaultagent_core::UnlockStatus;

use crate::agent_auth::authorized_vaults;
use crate::agent_unlock::with_env;
use crate::context::Context;
use crate::error::Result;
use crate::http::Response;
use crate::session::SessionStatus;
use crate::vaults::{VaultContext, Vaults};

/// Réponse de `status` : coffre, état de la session puis état des demandes de déverrouillage.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusReply {
    vault_id: String,
    vault_name: String,
    #[serde(flatten)]
    session: SessionStatus,
    #[serde(flatten)]
    unlock: UnlockStatus,
}

/// Réponse de `lock`.
#[derive(Serialize)]
struct Locked {
    locked: bool,
}

/// `vaults` : coffres où le jeton est valable, avec leur état.
///
/// Paramètres : le contexte, l'en-tête `authorization`.
/// Retour : la liste des coffres autorisés.
#[must_use]
pub fn vaults(context: &Context, header: Option<&str>) -> Response {
    let list: Vec<_> = authorized_vaults(context, header)
        .iter()
        .map(|vault| Vaults::metadata_of(vault))
        .collect();
    Response::json(200, &list)
}

/// `status` : état du coffre, sécurité d'approbation, demande du client et demandes à approuver.
///
/// Paramètres : le contexte, le coffre, l'appareil authentifié.
/// Retour : l'état.
///
/// # Errors
///
/// Erreur de lecture de la politique d'approbation.
pub fn status(context: &Context, vault: &VaultContext, device: &str) -> Result<Response> {
    let name = vault.name();
    let unlock = with_env(
        context,
        vault,
        |approvals, env| Ok(approvals.status(device, &name, env)),
    )?;
    Ok(Response::json(
        200,
        &StatusReply {
            vault_id: vault.id.clone(),
            vault_name: name,
            session: vault.session.status(),
            unlock,
        },
    ))
}

/// `lock` : verrouille le coffre.
///
/// Paramètres : le coffre.
/// Retour : `{locked:true}`.
#[must_use]
pub fn lock(vault: &VaultContext) -> Response {
    vault.session.lock();
    Response::json(200, &Locked { locked: true })
}
