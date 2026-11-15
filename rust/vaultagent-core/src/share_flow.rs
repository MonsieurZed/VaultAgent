//! Ouverture d'un partage dans le registre d'idempotence `share-receipts.json` : empreinte de la requête entière,
//! rejeu du reçu, conflit, intention sans reçu (`share-indeterminate`). La suite (capacité, intention, reçu)
//! passe par `ledger_flow` avec `Registry::Share`.

use crate::error::Result;
use crate::fingerprint::fingerprint;
use crate::ledger::{Ledger, Receipt, Registry};
use crate::ledger_flow::{OpenedOperation, open, replayed};
use crate::share_request::{ShareAction, ShareRequest};

/// Début d'un partage selon le registre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareStart {
    /// Aperçu : jamais inscrit au registre.
    Preview,
    /// Opération déjà exécutée : le reçu enregistré, marqué `replayed`.
    Replayed(Receipt),
    /// Opération neuve : à poursuivre avec `ensure_capacity` puis `persist_intent`.
    Fresh(OpenedOperation),
}

/// Ouvre un partage.
///
/// Paramètres : la clé machine, le profil du coffre, la requête, le registre de partage chargé.
/// Retour : l'état de départ.
/// # Errors
///
/// `operation-conflict` si l'opération existe avec une autre empreinte ; `share-indeterminate` si l'intention
/// existe sans reçu.
pub fn begin(machine_key: &[u8; 32], profile: &str, request: &ShareRequest, ledger: &Ledger) -> Result<ShareStart> {
    let Some(operation_id) = request
        .operation_id()
        .filter(|_| request.action() != ShareAction::Preview)
    else {
        return Ok(ShareStart::Preview);
    };
    let print = fingerprint(machine_key, request.raw());
    let opened = open(profile, operation_id, &print, ledger);
    Ok(match replayed(Registry::Share, &opened)? {
        Some(receipt) => ShareStart::Replayed(receipt),
        None => ShareStart::Fresh(opened),
    })
}
