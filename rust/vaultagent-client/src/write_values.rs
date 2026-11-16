//! Valeurs secrètes d'une écriture : lecture bornée de l'entrée standard, clés admises, effacement à la destruction.

use std::fmt;
use std::io::Read;

use serde::Deserialize;
use zeroize::{Zeroize, Zeroizing};

use crate::error::ClientError;

/// Taille maximale de l'entrée standard (64 Kio).
pub const MAX_VALUES_BYTES: usize = 64 * 1024;

/// Valeurs secrètes d'une fiche : sous-ensemble de `{username, password, totp, notes}`.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SecretValues {
    username: Option<String>,
    password: Option<String>,
    totp: Option<String>,
    notes: Option<String>,
}

impl fmt::Debug for SecretValues {
    /// Écrit seulement la présence de chaque champ, jamais sa valeur.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretValues")
            .field("username", &self.username.is_some())
            .field("password", &self.password.is_some())
            .field("totp", &self.totp.is_some())
            .field("notes", &self.notes.is_some())
            .finish()
    }
}

impl Drop for SecretValues {
    /// Efface chaque valeur de la mémoire.
    fn drop(&mut self) {
        for text in [&mut self.username, &mut self.password, &mut self.totp, &mut self.notes]
            .into_iter()
            .flatten()
        {
            text.zeroize();
        }
    }
}

impl SecretValues {
    /// Lit et valide l'objet JSON fourni sur l'entrée standard.
    ///
    /// Paramètres : le flux d'entrée.
    /// Retour : les valeurs, ou `Usage` si l'entrée dépasse 64 Kio, n'est pas un objet JSON ou porte une autre clé
    /// que `username`, `password`, `totp`, `notes`. Le message ne reprend jamais le contenu lu.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    pub fn read(input: &mut dyn Read) -> Result<Self, ClientError> {
        let mut raw = Zeroizing::new(Vec::with_capacity(MAX_VALUES_BYTES + 1));
        input
            .take(MAX_VALUES_BYTES as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|_| ClientError::Usage("Standard input unreadable.".to_owned()))?;
        if raw.len() > MAX_VALUES_BYTES {
            return Err(ClientError::Usage("Values larger than 64 KiB refused.".to_owned()));
        }
        serde_json::from_slice(&raw).map_err(|_| {
            ClientError::Usage("Values must be a JSON object with username, password, totp or notes only.".to_owned())
        })
    }

    /// Nom d'utilisateur, s'il est fourni.
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    /// Mot de passe, s'il est fourni.
    #[must_use]
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// Graine TOTP, si elle est fournie.
    #[must_use]
    pub fn totp(&self) -> Option<&str> {
        self.totp.as_deref()
    }

    /// Notes, si elles sont fournies.
    #[must_use]
    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    /// Indique si un champ de connexion (`username`, `password`, `totp`) est présent.
    #[must_use]
    pub fn has_login_field(&self) -> bool {
        self.username.is_some() || self.password.is_some() || self.totp.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subset_of_keys_is_read() {
        let values = SecretValues::read(&mut &b"{\"password\":\"p\",\"notes\":\"n\"}"[..]).unwrap();
        assert_eq!(values.password(), Some("p"));
        assert_eq!(values.notes(), Some("n"));
        assert!(values.username().is_none() && values.totp().is_none());
        assert!(values.has_login_field());
    }

    #[test]
    fn unknown_keys_and_bad_shapes_are_refused_without_echo() {
        for bad in [
            &b"{\"token\":\"leak-me\"}"[..],
            b"[\"leak-me\"]",
            b"{\"password\":12}",
            b"not json leak-me",
            b"",
        ] {
            let error = SecretValues::read(&mut &bad[..]).unwrap_err();
            assert_eq!(error.code(), "usage");
            assert!(!error.message().contains("leak-me"));
        }
    }

    #[test]
    fn oversized_input_is_refused() {
        let mut big = b"{\"notes\":\"".to_vec();
        big.extend(std::iter::repeat_n(b'a', MAX_VALUES_BYTES));
        big.extend_from_slice(b"\"}");
        assert!(SecretValues::read(&mut &big[..]).is_err());
    }

    #[test]
    fn debug_never_shows_values() {
        let values = SecretValues::read(&mut &b"{\"password\":\"sentinel-value\"}"[..]).unwrap();
        assert!(!format!("{values:?}").contains("sentinel-value"));
    }
}
