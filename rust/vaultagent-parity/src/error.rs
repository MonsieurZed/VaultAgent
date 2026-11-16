//! Erreur unique de l'outil de parité.

use std::fmt;

/// Erreur de l'outil : usage, réseau, protocole HTTP ou cryptographie du cœur.
#[derive(Debug)]
pub enum ParityError {
    /// Arguments de ligne de commande invalides.
    Usage(String),
    /// Échec de lecture ou d'écriture réseau.
    Io(String),
    /// Réponse HTTP illisible.
    Http(String),
    /// Erreur de `vaultagent-core` (clés, signatures, enveloppes).
    Core(String),
}

impl fmt::Display for ParityError {
    /// Rend le message d'erreur lisible.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(text) => write!(f, "usage : {text}"),
            Self::Io(text) => write!(f, "réseau : {text}"),
            Self::Http(text) => write!(f, "http : {text}"),
            Self::Core(text) => write!(f, "cœur : {text}"),
        }
    }
}

impl std::error::Error for ParityError {}

impl From<std::io::Error> for ParityError {
    /// Convertit une erreur d'entrée-sortie.
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<vaultagent_core::ErrorCode> for ParityError {
    /// Convertit un code d'erreur du cœur.
    fn from(error: vaultagent_core::ErrorCode) -> Self {
        Self::Core(error.to_string())
    }
}

/// Résultat de l'outil.
pub type Result<T> = std::result::Result<T, ParityError>;
