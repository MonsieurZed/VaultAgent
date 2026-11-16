//! Organisations actives et collections déchiffrées, droit d'écrire dans une collection : la logique pure
//! (`assemble`, `check`) est séparée de la lecture de l'état du SDK (`organizations`, `check_sharing`).

use bitwarden_collections::collection::CollectionView;
use bitwarden_vault::VaultClientExt as _;
use vaultagent_core::{Result, Uuid};

use crate::engine::stage_error;
use crate::sdk::session::Session;
use crate::sdk::sync_handler::OrgRecord;
use crate::structure::{Collection, Organization};

/// Indique si l'utilisateur peut écrire des fiches dans une collection.
///
/// Paramètres : la collection déchiffrée.
/// Retour : vrai si elle n'est pas en lecture seule ou si l'utilisateur la gère.
#[must_use]
pub fn can_edit(view: &CollectionView) -> bool {
    view.manage || !view.read_only
}

/// Associe chaque organisation active à ses collections.
///
/// Paramètres : les organisations actives et toutes les collections déchiffrées.
/// Retour : les organisations avec leurs collections et le droit d'édition.
#[must_use]
pub fn assemble(organizations: &[OrgRecord], collections: &[CollectionView]) -> Vec<Organization> {
    organizations
        .iter()
        .map(|org| Organization {
            id: org.id.clone(),
            name: org.name.clone(),
            role: org.role,
            collections: collections
                .iter()
                .filter(|view| view.organization_id.to_string() == org.id)
                .filter_map(|view| {
                    view.id.map(|id| Collection {
                        id: id.to_string(),
                        name: view.name.clone(),
                        can_edit: can_edit(view),
                    })
                })
                .collect(),
        })
        .collect()
}

/// Vérifie qu'une organisation active existe et que chaque collection visée lui appartient et est éditable.
///
/// Paramètres : les organisations assemblées, l'organisation et les collections visées.
///
/// # Errors
///
/// `sharing:refused` si l'organisation est inconnue, si aucune collection n'est visée ou si l'une est absente
/// ou en lecture seule.
pub fn check(organizations: &[Organization], organization_id: &Uuid, collection_ids: &[Uuid]) -> Result<()> {
    let refused = || stage_error("sharing", "refused");
    let org = organizations
        .iter()
        .find(|org| org.id == organization_id.as_str())
        .ok_or_else(refused)?;
    if collection_ids.is_empty() {
        return Err(refused());
    }
    let allowed = collection_ids.iter().all(|wanted| {
        org.collections
            .iter()
            .any(|collection| collection.id == wanted.as_str() && collection.can_edit)
    });
    allowed.then_some(()).ok_or_else(refused)
}

/// Organisations actives de l'utilisateur, collections déchiffrées par les clés d'organisation.
///
/// Paramètres : la session.
/// Retour : les organisations ; une collection qui ne se déchiffre pas est omise.
pub fn organizations(session: &Session) -> Vec<Organization> {
    let decrypted = session
        .client
        .vault()
        .collections()
        .decrypt_list_with_failures(session.orgs.collections());
    assemble(&session.orgs.organizations(), &decrypted.successes)
}

/// Vérifie le droit d'écrire dans chaque collection d'une organisation.
///
/// Paramètres : la session, l'organisation et les collections visées.
///
/// # Errors
///
/// `sharing:refused`.
pub fn check_sharing(session: &Session, organization_id: &Uuid, collection_ids: &[Uuid]) -> Result<()> {
    check(&organizations(session), organization_id, collection_ids)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bitwarden_collections::collection::CollectionType;

    use super::*;

    const ORG: &str = "22222222-2222-4222-8222-222222222222";
    const EDITABLE: &str = "33333333-3333-4333-8333-333333333333";
    const READ_ONLY: &str = "44444444-4444-4444-8444-444444444444";

    /// Collection déchiffrée factice.
    fn view(id: &str, organization: &str, read_only: bool) -> CollectionView {
        CollectionView {
            id: id.parse().ok(),
            organization_id: organization.parse().unwrap(),
            name: format!("Collection {id}"),
            external_id: None,
            hide_passwords: false,
            read_only,
            manage: false,
            r#type: CollectionType::SharedCollection,
            has_enabled_access_rule: None,
        }
    }

    /// Organisations assemblées de test.
    fn sample() -> Vec<Organization> {
        let records = [OrgRecord {
            id: ORG.into(),
            name: "Org factice".into(),
            role: 2,
        }];
        let other = "55555555-5555-4555-8555-555555555555";
        assemble(
            &records,
            &[
                view(EDITABLE, ORG, false),
                view(READ_ONLY, ORG, true),
                view(EDITABLE, other, false),
            ],
        )
    }

    #[test]
    fn collections_are_grouped_by_organization_with_edit_rights() {
        let orgs = sample();
        assert_eq!(orgs.len(), 1);
        assert_eq!(orgs[0].collections.len(), 2);
        assert_eq!(
            orgs[0].collections.iter().map(|c| c.can_edit).collect::<Vec<_>>(),
            [true, false]
        );
    }

    #[test]
    fn sharing_is_refused_without_edit_rights_or_target() {
        let orgs = sample();
        let org = Uuid::parse(ORG).unwrap();
        let ok = Uuid::parse(EDITABLE).unwrap();
        let ro = Uuid::parse(READ_ONLY).unwrap();
        assert!(check(&orgs, &org, std::slice::from_ref(&ok)).is_ok());
        for (organization, targets) in [
            (&org, vec![ro.clone()]),
            (&org, vec![ok.clone(), ro]),
            (&org, vec![]),
            (&Uuid::parse("66666666-6666-4666-8666-666666666666").unwrap(), vec![ok]),
        ] {
            let code = check(&orgs, organization, &targets).err().map(|e| e.to_string());
            assert_eq!(code.as_deref(), Some("sharing:refused"));
        }
    }
}
