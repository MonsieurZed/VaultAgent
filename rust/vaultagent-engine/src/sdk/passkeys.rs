//! Passkeys : liste des identifiants FIDO2 des fiches de connexion et création d'une fiche qui en porte une, relue
//! pour vérifier le stockage. Chemin natif, sans la fonctionnalité `wasm` : l'identifiant déchiffré
//! (`Fido2CredentialView`) se pose directement dans la `LoginView`, que le SDK chiffre à la création.

use bitwarden_vault::{
    CipherCreateRequest, CipherRepromptType, CipherView, CipherViewType, Fido2CredentialView, LoginUriView, LoginView,
    VaultClientExt as _,
};
use serde_json::Value;
use vaultagent_core::Result;

use crate::engine::stage_error;
use crate::passkey::{Passkey, PasskeyEntry};
use crate::sdk::clock::iso_now;
use crate::sdk::read::read_error;
use crate::sdk::session::Session;
use crate::sdk::write_errors::create_error;

/// Passkey du moteur depuis un identifiant FIDO2 du SDK.
///
/// Paramètres : l'identifiant déchiffré.
/// Retour : la passkey, `None` si la conversion échoue.
fn passkey_from_view(view: &Fido2CredentialView) -> Option<Passkey> {
    serde_json::to_value(view)
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
}

/// Identifiant FIDO2 du SDK depuis une passkey du moteur.
///
/// Paramètres : la passkey et la date à poser si elle n'en a pas.
/// Retour : l'identifiant.
///
/// # Errors
///
/// `passkeys:rejected` si une date est illisible.
fn credential_view(passkey: &Passkey, now: &str) -> Result<Fido2CredentialView> {
    let rejected = || stage_error("passkeys", "rejected");
    let mut json = serde_json::to_value(passkey).map_err(|_| rejected())?;
    if let Some(map) = json.as_object_mut() {
        let date = passkey.creation_date.clone().unwrap_or_else(|| now.to_owned());
        map.insert("creationDate".to_owned(), Value::String(date));
    }
    serde_json::from_value(json).map_err(|_| rejected())
}

/// Passkeys des fiches actives, filtrées par partie de confiance.
///
/// Paramètres : les fiches déchiffrées et le domaine éventuel.
/// Retour : une entrée par identifiant, dans l'ordre des fiches ; corbeille et archives exclues.
#[must_use]
pub fn entries(views: &[CipherView], rp_id: Option<&str>) -> Vec<PasskeyEntry> {
    views
        .iter()
        .filter(|view| view.deleted_date.is_none() && view.archived_date.is_none())
        .flat_map(|view| {
            let id = view.id.map_or_else(String::new, |id| id.to_string());
            let found = view.login.iter().filter_map(|login| login.fido2_credentials.as_ref());
            found.flatten().map(move |credential| (id.clone(), credential))
        })
        .filter(|(_, credential)| rp_id.is_none_or(|wanted| credential.rp_id == wanted))
        .filter_map(|(item_id, credential)| {
            passkey_from_view(credential).map(|credential| PasskeyEntry { item_id, credential })
        })
        .collect()
}

/// Liste les passkeys du coffre.
///
/// Paramètres : la session et le domaine éventuel.
/// Retour : les passkeys correspondantes.
///
/// # Errors
///
/// `items:rejected` si le dépôt est illisible.
pub async fn list(session: &Session, rp_id: Option<&str>) -> Result<Vec<PasskeyEntry>> {
    let all = session
        .client
        .vault()
        .ciphers()
        .get_all()
        .await
        .map_err(|e| read_error(&e))?;
    Ok(entries(&all.successes, rp_id))
}

/// Vérifie qu'une fiche relue porte la passkey attendue.
///
/// Paramètres : la fiche relue et la passkey créée.
///
/// # Errors
///
/// `passkeys:passkey-storage` si `credentialId`, `keyValue` ou `rpId` diffèrent ou manquent.
pub fn verify_stored(view: &CipherView, passkey: &Passkey) -> Result<()> {
    let stored = view
        .login
        .iter()
        .filter_map(|login| login.fido2_credentials.as_ref())
        .flatten()
        .find(|credential| credential.credential_id == passkey.credential_id);
    match stored {
        Some(found) if found.key_value == passkey.key_value.expose() && found.rp_id == passkey.rp_id => Ok(()),
        _ => Err(stage_error("passkeys", "passkey-storage")),
    }
}

/// Requête de création de la fiche qui porte la passkey.
///
/// Paramètres : la passkey et la date de repli.
/// Retour : la requête d'une connexion vers `https://<rpId>`.
///
/// # Errors
///
/// `passkeys:rejected`.
fn create_request(passkey: &Passkey, now: &str) -> Result<CipherCreateRequest> {
    let label = passkey.rp_name.as_deref().unwrap_or(&passkey.rp_id);
    let user = passkey.user_name.as_deref().unwrap_or("Passkey");
    let login = LoginView {
        username: passkey.user_name.clone(),
        password: None,
        password_revision_date: None,
        uris: Some(vec![LoginUriView {
            uri: Some(format!("https://{}", passkey.rp_id)),
            r#match: None,
            uri_checksum: None,
        }]),
        totp: None,
        autofill_on_page_load: None,
        http_authentication_enabled: None,
        fido2_credentials: Some(vec![credential_view(passkey, now)?]),
    };
    Ok(CipherCreateRequest {
        organization_id: None,
        collection_ids: Vec::new(),
        folder_id: None,
        name: format!("{label} \u{2014} {user}"),
        notes: None,
        favorite: false,
        reprompt: CipherRepromptType::None,
        r#type: CipherViewType::Login(login),
        fields: Vec::new(),
        archived_date: None,
    })
}

/// Crée la fiche qui porte la passkey, la relit et vérifie le stockage.
///
/// Paramètres : la session et la passkey complète.
/// Retour : l'identifiant de la fiche créée.
///
/// # Errors
///
/// `passkeys:passkey-storage` si la relecture diffère, `passkeys:*` sinon.
pub async fn create(session: &Session, passkey: &Passkey) -> Result<String> {
    let ciphers = session.client.vault().ciphers();
    let created = ciphers
        .create(create_request(passkey, &iso_now())?)
        .await
        .map_err(|e| create_error("passkeys", &e))?;
    let id = created.id.ok_or_else(|| stage_error("passkeys", "rejected"))?;
    let saved = ciphers
        .get(id)
        .await
        .map_err(|_| stage_error("passkeys", "passkey-storage"))?;
    verify_stored(&saved, passkey)?;
    Ok(id.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::secret::Secret;

    /// Passkey factice du domaine donné.
    fn passkey(rp_id: &str, credential: &str) -> Passkey {
        Passkey {
            credential_id: credential.into(),
            key_type: "public-key".into(),
            key_algorithm: "ECDSA".into(),
            key_curve: "P-256".into(),
            key_value: Secret::from("fake-key-value"),
            rp_id: rp_id.into(),
            rp_name: Some("Exemple".into()),
            user_handle: Some("dXNlcg".into()),
            user_name: Some("alice".into()),
            user_display_name: None,
            counter: "0".into(),
            discoverable: "true".into(),
            creation_date: Some("2026-11-16T10:00:00Z".into()),
        }
    }

    /// Fiche déchiffrée portant les passkeys données, éventuellement à la corbeille.
    fn view(passkeys: &[Passkey], deleted: bool) -> CipherView {
        let credentials: Vec<Fido2CredentialView> = passkeys
            .iter()
            .map(|p| credential_view(p, "2026-01-01T00:00:00Z").unwrap())
            .collect();
        serde_json::from_value(json!({
            "id": "11111111-1111-4111-8111-111111111111", "organizationId": null, "folderId": null,
            "collectionIds": [], "key": null, "name": "Fiche", "notes": null, "type": 1,
            "login": {"username": null, "password": null, "passwordRevisionDate": null, "uris": null,
                "totp": null, "autofillOnPageLoad": null, "httpAuthenticationEnabled": null,
                "fido2Credentials": credentials},
            "identity": null, "card": null, "secureNote": null, "sshKey": null, "bankAccount": null,
            "driversLicense": null, "passport": null, "favorite": false, "reprompt": 0,
            "organizationUseTotp": false, "edit": true, "permissions": null, "viewPassword": true,
            "localData": null, "attachments": null, "fields": null, "passwordHistory": null,
            "creationDate": "2026-01-01T00:00:00Z",
            "deletedDate": if deleted { json!("2026-02-01T00:00:00Z") } else { Value::Null },
            "revisionDate": "2026-11-16T10:00:00Z", "archivedDate": null
        }))
        .unwrap()
    }

    #[test]
    fn passkeys_round_trip_through_the_sdk_credential() {
        let original = passkey("example.invalid", "cred-1");
        let back = passkey_from_view(&credential_view(&original, "x").unwrap()).unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn a_missing_creation_date_takes_the_fallback() {
        let mut bare = passkey("example.invalid", "cred-1");
        bare.creation_date = None;
        let back = passkey_from_view(&credential_view(&bare, "2026-03-04T05:06:07Z").unwrap()).unwrap();
        assert_eq!(back.creation_date.as_deref(), Some("2026-03-04T05:06:07Z"));
    }

    #[test]
    fn the_list_filters_by_relying_party_and_skips_the_trash() {
        let both = [passkey("a.example.invalid", "c1"), passkey("b.example.invalid", "c2")];
        let views = [view(&both, false), view(&[passkey("a.example.invalid", "c3")], true)];
        assert_eq!(entries(&views, None).len(), 2);
        let only_a = entries(&views, Some("a.example.invalid"));
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].credential.credential_id, "c1");
        assert_eq!(only_a[0].item_id, "11111111-1111-4111-8111-111111111111");
        assert_eq!(entries(&views, Some("absent.invalid")).len(), 0);
    }

    #[test]
    fn storage_is_verified_on_id_key_and_domain() {
        let wanted = passkey("a.example.invalid", "c1");
        let stored = view(std::slice::from_ref(&wanted), false);
        assert!(verify_stored(&stored, &wanted).is_ok());
        let mut other_key = wanted.clone();
        other_key.key_value = Secret::from("autre-cle");
        let mut other_rp = wanted.clone();
        other_rp.rp_id = "b.example.invalid".into();
        let mut other_id = wanted.clone();
        other_id.credential_id = "inconnu".into();
        for changed in [other_key, other_rp, other_id] {
            let code = verify_stored(&stored, &changed).err().map(|e| e.to_string());
            assert_eq!(code.as_deref(), Some("passkeys:passkey-storage"));
        }
    }

    #[test]
    fn the_created_item_targets_the_relying_party() {
        let request = create_request(&passkey("a.example.invalid", "c1"), "x").unwrap();
        assert_eq!(request.name, "Exemple \u{2014} alice");
        let CipherViewType::Login(login) = &request.r#type else {
            unreachable!()
        };
        assert_eq!(
            login.uris.as_ref().and_then(|u| u[0].uri.as_deref()),
            Some("https://a.example.invalid")
        );
        assert_eq!(login.fido2_credentials.as_ref().map(Vec::len), Some(1));
    }
}
