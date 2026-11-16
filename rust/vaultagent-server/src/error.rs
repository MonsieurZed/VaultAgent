//! Erreur unique de la crate serveur : codes du protocole de `vaultagent-core` plus les codes que Node émet
//! depuis le magasin et la session (`private-key-*`, `invalid-account`, `cancelled`…).

use std::fmt;

use vaultagent_core::ErrorCode;

/// Erreur du magasin, de la session et des coffres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerError {
    /// Code du protocole ou du moteur.
    Code(ErrorCode),
    /// `machine.key` lisible par le groupe ou les autres.
    PrivateKeyPermissions,
    /// `machine.key` d'une taille autre que 32 octets.
    PrivateKeyInvalid,
    /// Serveur ou courriel refusé par `configure`.
    InvalidAccount,
    /// Délai d'inactivité hors de 1 à 1440 minutes.
    InvalidTimeout,
    /// Mot de passe absent, vide ou de plus de 8192 unités UTF-16.
    PasswordRequired,
    /// Opération abandonnée parce que la session a été verrouillée pendant la connexion.
    Cancelled,
    /// Échec d'entrée-sortie (message du système).
    Io(String),
    /// Fichier JSON illisible ou valeur non sérialisable (message de `serde_json`).
    Json(String),
    /// Réglage de démarrage absent ou invalide (variable d'environnement).
    Settings(String),
    /// Ouverture du port d'écoute impossible (message du système).
    Listen(String),
}

impl ServerError {
    /// Code envoyé aux clients dans `{"error": code}` ; les échecs internes se replient sur `request-refused`.
    ///
    /// Paramètres : aucun.
    /// Retour : le texte du code.
    #[must_use]
    pub fn code_text(&self) -> &str {
        match self {
            Self::Code(code) => code.as_str(),
            Self::PrivateKeyPermissions => "private-key-permissions",
            Self::PrivateKeyInvalid => "private-key-invalid",
            Self::InvalidAccount => "invalid-account",
            Self::InvalidTimeout => "invalid-timeout",
            Self::PasswordRequired => "password-required",
            Self::Cancelled => "cancelled",
            Self::Io(_) | Self::Json(_) | Self::Settings(_) | Self::Listen(_) => "request-refused",
        }
    }

    /// Convertit en code du protocole pour les traits de `vaultagent-core`, qui ne connaissent que `ErrorCode`.
    ///
    /// Paramètres : aucun.
    /// Retour : le code d'origine, ou `request-refused` pour les erreurs propres au serveur.
    #[must_use]
    pub fn into_code(self) -> ErrorCode {
        match self {
            Self::Code(code) => code,
            _ => ErrorCode::RequestRefused,
        }
    }
}

impl fmt::Display for ServerError {
    /// Écrit le code ; les messages système restent hors de la sortie, sauf ceux du démarrage.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Settings(message) | Self::Listen(message) => f.write_str(message),
            _ => f.write_str(self.code_text()),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<ErrorCode> for ServerError {
    /// Enveloppe un code du protocole.
    fn from(code: ErrorCode) -> Self {
        Self::Code(code)
    }
}

impl From<std::io::Error> for ServerError {
    /// Garde le message du système.
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<serde_json::Error> for ServerError {
    /// Garde le message de `serde_json`.
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}

/// Résultat des fonctions de la crate.
pub type Result<T> = std::result::Result<T, ServerError>;
