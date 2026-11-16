//! Conversion d'une fiche déchiffrée du SDK vers une requête d'édition, par le patch de `vaultagent-core` : la fiche
//! passe en JSON (`CipherView`), `apply_write_patch` contrôle la révision et applique le patch, puis le résultat
//! redevient un `CipherView` et une `CipherEditRequest`. Logique pure, sans réseau.

use bitwarden_vault::{CipherEditRequest, CipherView};
use serde_json::Value;
use vaultagent_core::{ErrorCode, Result, RevisionDate, Uuid, apply_write_patch};

use crate::engine::stage_error;

/// Refus de l'écriture : fiche non modifiable par l'utilisateur ou forme inattendue.
fn rejected() -> ErrorCode {
    stage_error("write", "rejected")
}

/// Construit la requête d'édition d'une fiche relue, patch appliqué.
///
/// Paramètres : la fiche relue après resynchronisation, la révision attendue par l'appelant, le patch, l'organisation
/// de la fiche (`None` si personnelle) et le drapeau de la politique `OrganizationDataOwnership`.
/// Retour : la requête, dont la `revision_date` est celle de la fiche relue.
///
/// # Errors
///
/// `write:rejected` si l'utilisateur ne peut pas éditer la fiche ou si une fiche personnelle est interdite par la
/// politique ; `RevisionConflict` si la révision diffère ; `InvalidPatch` et `ItemRefused` du patch.
pub fn edit_request(
    view: &CipherView,
    expected_revision: &RevisionDate,
    patch: &Value,
    organization_id: Option<&Uuid>,
    data_ownership: bool,
) -> Result<CipherEditRequest> {
    if !view.edit || (view.organization_id.is_none() && data_ownership) {
        return Err(rejected());
    }
    let mut json = serde_json::to_value(view).map_err(|_| rejected())?;
    apply_write_patch(&mut json, expected_revision, patch, organization_id)?;
    let patched: CipherView = serde_json::from_value(json).map_err(|_| rejected())?;
    CipherEditRequest::try_from(patched).map_err(|_| rejected())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use bitwarden_vault::CipherViewType;

    use super::*;

    const REVISION: &str = "2026-11-16T10:00:00.123456Z";

    /// Fiche de connexion factice, éditable, à la révision `REVISION`.
    fn view(edit: bool, organization: Option<&str>) -> CipherView {
        serde_json::from_value(json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "organizationId": organization, "folderId": null, "collectionIds": [], "key": null,
            "name": "Fiche factice", "notes": null, "type": 1,
            "login": {"username": "alice", "password": "factice-mdp", "passwordRevisionDate": null,
                "uris": [{"uri": "https://a.example.invalid", "match": null, "uriChecksum": null}],
                "totp": null, "autofillOnPageLoad": null, "httpAuthenticationEnabled": null,
                "fido2Credentials": null},
            "identity": null, "card": null, "secureNote": null, "sshKey": null, "bankAccount": null,
            "driversLicense": null, "passport": null, "favorite": false, "reprompt": 0,
            "organizationUseTotp": false, "edit": edit, "permissions": null, "viewPassword": true,
            "localData": null, "attachments": null, "fields": null, "passwordHistory": null,
            "creationDate": "2026-01-01T00:00:00Z", "deletedDate": null,
            "revisionDate": REVISION, "archivedDate": null
        }))
        .unwrap()
    }

    /// Révision attendue.
    fn revision(text: &str) -> RevisionDate {
        RevisionDate::parse(text).unwrap()
    }

    /// Texte du code d'erreur d'un résultat.
    fn code<T>(result: Result<T>) -> String {
        result.err().map_or_default(|e| e.to_string())
    }

    #[test]
    fn a_patch_becomes_an_edit_request_that_keeps_other_fields() {
        let patch = json!({"name": "Renommée", "addUris": ["https://b.example.invalid"]});
        let request = edit_request(&view(true, None), &revision(REVISION), &patch, None, false).unwrap();
        assert_eq!(request.name, "Renommée");
        let CipherViewType::Login(login) = &request.r#type else {
            unreachable!()
        };
        assert_eq!(login.username.as_deref(), Some("alice"));
        assert_eq!(login.password.as_deref(), Some("factice-mdp"));
        assert_eq!(login.uris.as_ref().map(Vec::len), Some(2));
        assert_eq!(request.revision_date.to_rfc3339(), "2026-11-16T10:00:00.123456+00:00");
    }

    #[test]
    fn an_old_revision_is_a_conflict() {
        let patch = json!({"name": "x"});
        let old = revision("2020-01-01T00:00:00.000Z");
        let result = edit_request(&view(true, None), &old, &patch, None, false);
        assert_eq!(code(result), "revision-conflict");
    }

    #[test]
    fn edit_right_policy_owner_and_patch_are_enforced() {
        let patch = json!({"name": "x"});
        let rev = revision(REVISION);
        assert_eq!(
            code(edit_request(&view(false, None), &rev, &patch, None, false)),
            "write:rejected"
        );
        assert_eq!(
            code(edit_request(&view(true, None), &rev, &patch, None, true)),
            "write:rejected"
        );
        let other = Uuid::parse("22222222-2222-4222-8222-222222222222").unwrap();
        assert_eq!(
            code(edit_request(&view(true, None), &rev, &patch, Some(&other), false)),
            "item-refused"
        );
        assert_eq!(
            code(edit_request(&view(true, None), &rev, &json!({"type": 2}), None, false)),
            "invalid-patch"
        );
    }

    #[test]
    fn an_organization_item_is_edited_with_its_organization() {
        let org = "22222222-2222-4222-8222-222222222222";
        let id = Uuid::parse(org).unwrap();
        let patch = json!({"notes": "note"});
        let request = edit_request(&view(true, Some(org)), &revision(REVISION), &patch, Some(&id), true).unwrap();
        assert_eq!(request.notes.as_deref(), Some("note"));
        assert!(request.organization_id.is_some());
    }
}
