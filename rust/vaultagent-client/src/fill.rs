//! Remplissage par enveloppe : clé RSA éphémère, `seal`, ouverture locale et remise de la valeur à un puits.

use serde::Serialize;
use vaultagent_core::{Envelope, RsaPrivateKey, SystemEntropy, generate_rsa, open_envelope, public_key_spki};
use zeroize::Zeroizing;

use crate::config::ClientConfig;
use crate::error::ClientError;
use crate::seal::{check_target, request_envelope, seal_body};
use crate::transport::Transport;

/// Taille du module de la clé éphémère, en bits.
const KEY_BITS: usize = 2048;

/// Résultat d'un remplissage ; il ne porte jamais la valeur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FillReport {
    /// Toujours vrai quand le puits a reçu la valeur.
    pub filled: bool,
    /// Champ rempli.
    pub field: String,
    /// Toujours faux : le presse-papiers n'est jamais utilisé.
    #[serde(rename = "clipboardUsed")]
    pub clipboard_used: bool,
}

/// Erreur rendue quand l'enveloppe ne peut pas être ouverte.
fn unreadable() -> ClientError {
    ClientError::BadResponse("Envelope could not be opened.".to_owned())
}

/// Remplit un champ : génère une paire RSA 2048 puis délègue à `fill_with_key`.
///
/// Paramètres : le transport, la configuration (jeton), le coffre choisi, la fiche, le champ, l'origine HTTPS exacte,
/// le puits qui reçoit la valeur en clair (il ne doit ni la journaliser ni la renvoyer).
/// Retour : `{filled:true, field, clipboardUsed:false}` ; la clé privée et la valeur sont effacées à la sortie.
///
/// # Errors
///
/// `Usage` pour un argument invalide ; l'erreur classée du serveur ; `BadResponse` si l'enveloppe est illisible ;
/// l'erreur du puits.
pub fn fill_with<F>(
    transport: &dyn Transport,
    config: &ClientConfig,
    vault: Option<&str>,
    item: &str,
    field: &str,
    origin: &str,
    sink: F,
) -> Result<FillReport, ClientError>
where
    F: FnOnce(&Zeroizing<String>) -> Result<(), ClientError>,
{
    check_target(vault, item, field, origin)?;
    let key = generate_rsa(KEY_BITS, &mut SystemEntropy).map_err(|_| unreadable())?;
    fill_with_key(transport, config, vault, (item, field, origin), &key, sink)
}

/// Remplit un champ avec une clé privée fournie : `seal`, ouverture de l'enveloppe, remise de la valeur au puits.
///
/// Paramètres : comme `fill_with`, avec la cible en triplet (fiche, champ, origine) et la clé RSA (2048 à 4096 bits).
/// Retour : le compte rendu du remplissage.
///
/// # Errors
///
/// Les erreurs possibles sont celles de `fill_with`.
pub fn fill_with_key<F>(
    transport: &dyn Transport,
    config: &ClientConfig,
    vault: Option<&str>,
    target: (&str, &str, &str),
    key: &RsaPrivateKey,
    sink: F,
) -> Result<FillReport, ClientError>
where
    F: FnOnce(&Zeroizing<String>) -> Result<(), ClientError>,
{
    let (item, field, origin) = target;
    let public = public_key_spki(key).map_err(|_| unreadable())?;
    let body = seal_body(vault, item, field, origin, &public)?;
    let line = request_envelope(transport, &config.token, &body)?;
    let envelope: Envelope = serde_json::from_str(&line).map_err(|_| unreadable())?;
    let clear = open_envelope(&envelope, key, &mut SystemEntropy).map_err(|_| unreadable())?;
    let text = Zeroizing::new(std::str::from_utf8(&clear).map_err(|_| unreadable())?.to_owned());
    sink(&text)?;
    Ok(FillReport {
        filled: true,
        field: field.to_owned(),
        clipboard_used: false,
    })
}
