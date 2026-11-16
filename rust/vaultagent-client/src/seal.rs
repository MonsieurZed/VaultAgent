//! Commande `seal` : relaie la demande d'enveloppe chiffrée pour une clé publique, sans jamais ouvrir la valeur.

use clap::Args;
use serde::Serialize;
use vaultagent_core::{SystemEntropy, Uuid, seal};

use crate::api::agent_call;
use crate::error::ClientError;
use crate::server_address::ServerAddress;
use crate::token::Token;
use crate::transport::Transport;

/// Champs de fiche que le serveur accepte de sceller.
const FIELDS: [&str; 3] = ["username", "password", "totp"];

/// Arguments de `seal`.
#[derive(Debug, Clone, Args)]
pub struct SealArgs {
    /// Identifiant (UUID) de la fiche.
    #[arg(help = "Item id (UUID).")]
    pub item_id: String,
    /// Champ demandé : `username`, `password` ou `totp`.
    #[arg(help = "Field to seal: username, password or totp.")]
    pub field: String,
    /// Origine HTTPS exacte de la cible.
    #[arg(help = "Exact HTTPS origin of the target page, without path.")]
    pub origin: String,
    /// Clé publique RSA du destinataire, SPKI DER en base64.
    #[arg(
        long,
        value_name = "SPKI_BASE64",
        help = "RSA public key (SPKI DER, base64) of the only holder able to open the envelope."
    )]
    pub public_key: String,
}

/// Corps de la route `seal`.
#[derive(Serialize)]
pub struct SealBody<'a> {
    #[serde(rename = "vaultId", skip_serializing_if = "Option::is_none")]
    vault_id: Option<&'a str>,
    #[serde(rename = "itemId")]
    item_id: &'a str,
    field: &'a str,
    origin: &'a str,
    #[serde(rename = "publicKey")]
    public_key: &'a str,
}

/// Valide la cible d'une demande de scellement, sans réseau ni clé.
///
/// Paramètres : le coffre choisi, l'identifiant de fiche, le champ et l'origine.
/// Retour : rien si tout convient, sinon `Usage`.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn check_target(vault: Option<&str>, item_id: &str, field: &str, origin: &str) -> Result<(), ClientError> {
    if vault.is_some_and(str::is_empty) {
        return Err(ClientError::Usage("Vault id required after --vault.".to_owned()));
    }
    if Uuid::parse(item_id).is_none() {
        return Err(ClientError::Usage("Item id must be a UUID.".to_owned()));
    }
    if !FIELDS.contains(&field) {
        return Err(ClientError::Usage(
            "Field must be username, password or totp.".to_owned(),
        ));
    }
    let exact = ServerAddress::parse(origin).is_ok_and(|address| address.as_str() == origin);
    if exact {
        Ok(())
    } else {
        Err(ClientError::Usage("Exact HTTPS origin required.".to_owned()))
    }
}

/// Valide les quatre arguments d'une demande de scellement, sans réseau.
///
/// Paramètres : le coffre choisi, l'identifiant de fiche, le champ, l'origine et la clé publique.
/// Retour : le corps de la requête, ou `Usage` si un argument est invalide.
///
/// # Errors
///
/// Les erreurs possibles sont celles décrites dans Retour.
pub fn seal_body<'a>(
    vault: Option<&'a str>,
    item_id: &'a str,
    field: &'a str,
    origin: &'a str,
    public_key: &'a str,
) -> Result<SealBody<'a>, ClientError> {
    check_target(vault, item_id, field, origin)?;
    if seal(&[], public_key, &mut SystemEntropy).is_err() {
        return Err(ClientError::Usage(
            "Public key must be an RSA SPKI key (base64, 2048 to 4096 bits).".to_owned(),
        ));
    }
    Ok(SealBody {
        vault_id: vault,
        item_id,
        field,
        origin,
        public_key,
    })
}

/// Demande une enveloppe au serveur.
///
/// Paramètres : le transport, le jeton, le corps validé.
/// Retour : le corps JSON de l'enveloppe, tel que le serveur l'a rendu, ou l'erreur classée.
///
/// # Errors
///
/// Les erreurs possibles sont celles de `agent_call`.
pub fn request_envelope(transport: &dyn Transport, token: &Token, body: &SealBody<'_>) -> Result<String, ClientError> {
    agent_call(transport, token, "seal", body)
}

/// Exécute `seal` : valide, relaie, rend l'enveloppe chiffrée sans l'ouvrir.
///
/// Paramètres : le transport, le jeton, le coffre choisi, les arguments.
/// Retour : l'enveloppe `{encryptedKey, nonce, tag, data}` sur une ligne.
///
/// # Errors
///
/// `Usage` pour un argument invalide ; sinon l'erreur classée du serveur.
pub fn run_seal(
    transport: &dyn Transport,
    token: &Token,
    vault: Option<&str>,
    args: &SealArgs,
) -> Result<String, ClientError> {
    let body = seal_body(vault, &args.item_id, &args.field, &args.origin, &args.public_key)?;
    request_envelope(transport, token, &body)
}
