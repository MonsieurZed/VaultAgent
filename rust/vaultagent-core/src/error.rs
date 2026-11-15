//! Erreur unique de la crate : les codes du protocole `VaultAgent` (section « Erreurs » de `docs/rust/protocole.md`),
//! leur texte exact et leur statut HTTP. Toutes les fonctions de la crate rendent [`Result`].

use std::fmt;

macro_rules! error_codes {
    ($($variant:ident => $text:literal,)+) => {
        /// Code d'erreur du protocole ; `as_str` rend le texte exact envoyé aux clients.
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum ErrorCode {
            $(#[doc = concat!("Code du protocole `", $text, "`.")] $variant,)+
            /// Code relayé du moteur : `engine-*` ou `<module>:<raison>` (voir [`ErrorCode::engine`]).
            Engine(String),
        }

        impl ErrorCode {
            /// Texte exact du code, tel que rendu dans `{"error": code}`.
            #[must_use]
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $text,)+
                    Self::Engine(code) => code,
                }
            }

            /// Retrouve le code connu correspondant à un texte, hors codes du moteur.
            fn known(text: &str) -> Option<Self> {
                match text {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

error_codes! {
    Unauthorized => "unauthorized",
    OwnerRequired => "owner-required",
    ProxyRequired => "proxy-required",
    OriginRefused => "origin-refused",
    Locked => "locked",
    RequestRefused => "request-refused",
    VaultRequired => "vault-required",
    VaultMissing => "vault-missing",
    InvalidVaultRegistry => "invalid-vault-registry",
    InvalidVault => "invalid-vault",
    InvalidVaultName => "invalid-vault-name",
    VaultLimit => "vault-limit",
    DuplicateAccount => "duplicate-account",
    JsonRequired => "json-required",
    RequestSize => "request-size",
    RouteRefused => "route-refused",
    MethodRefused => "method-refused",
    NotConfigured => "not-configured",
    InvalidField => "invalid-field",
    FieldMissing => "field-missing",
    TotpPeriod => "totp-period",
    InvalidWrite => "invalid-write",
    InvalidPatch => "invalid-patch",
    InvalidItem => "invalid-item",
    ItemRefused => "item-refused",
    RevisionConflict => "revision-conflict",
    DuplicateItem => "duplicate-item",
    DuplicateFolder => "duplicate-folder",
    FolderMissing => "folder-missing",
    OperationConflict => "operation-conflict",
    ReceiptCapacity => "receipt-capacity",
    WriteIndeterminate => "write-indeterminate",
    InvalidShare => "invalid-share",
    OrganizationRefused => "organization-refused",
    DuplicateCollection => "duplicate-collection",
    ShareIndeterminate => "share-indeterminate",
    RequestExpired => "request-expired",
    ApproverRequired => "approver-required",
    ApprovalRefused => "approval-refused",
    ElevationRequired => "elevation-required",
    HelloRequired => "hello-required",
    HelloInvalid => "hello-invalid",
    HelloEnrollmentRefused => "hello-enrollment-refused",
    HelloEnrollmentAuthorizationRequired => "hello-enrollment-authorization-required",
    ElevationEnrollmentRefused => "elevation-enrollment-refused",
    PairingRefused => "pairing-refused",
    PairingRateLimit => "pairing-rate-limit",
    PairingLimit => "pairing-limit",
    InvalidDevice => "invalid-device",
    DeviceLimit => "device-limit",
    InvalidKey => "invalid-key",
}

/// Résultat de toutes les fonctions de la crate.
pub type Result<T> = std::result::Result<T, ErrorCode>;

/// Indique si un texte suit la forme `^[a-z-]+(?::[a-z-]+)?$` imposée aux codes par le protocole.
fn has_code_shape(text: &str) -> bool {
    let word = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b == b'-');
    match text.split_once(':') {
        Some((module, reason)) => word(module) && word(reason),
        None => word(text),
    }
}

impl ErrorCode {
    /// Construit un code relayé du moteur.
    ///
    /// Paramètres : le texte du code, qui doit suivre la forme du protocole et être `engine-*` ou `<module>:<raison>`.
    /// Retour : `Engine(code)`.
    /// # Errors
    ///
    /// `RequestRefused` si le texte n'a pas cette forme.
    pub fn engine(code: &str) -> Result<Self> {
        let relayed = code.starts_with("engine-") || code.contains(':');
        if has_code_shape(code) && relayed {
            Ok(Self::Engine(code.to_owned()))
        } else {
            Err(Self::RequestRefused)
        }
    }

    /// Interprète le texte d'un code reçu d'un serveur ou du moteur, selon la règle du protocole.
    ///
    /// Paramètres : le texte du code.
    /// Retour : le code connu, un code du moteur valide, ou `RequestRefused` pour tout autre texte.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        Self::known(text)
            .or_else(|| Self::engine(text).ok())
            .unwrap_or(Self::RequestRefused)
    }

    /// Statut HTTP associé : 403 pour les refus d'identité, 423 pour `locked`, 400 sinon.
    #[must_use]
    pub fn http_status(&self) -> u16 {
        match self {
            Self::Unauthorized | Self::OwnerRequired | Self::ProxyRequired | Self::OriginRefused => 403,
            Self::Locked => 423,
            _ => 400,
        }
    }
}

impl fmt::Display for ErrorCode {
    /// Écrit le texte exact du code.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for ErrorCode {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_render_protocol_text() {
        assert_eq!(ErrorCode::InvalidWrite.as_str(), "invalid-write");
        assert_eq!(
            ErrorCode::HelloEnrollmentAuthorizationRequired.to_string(),
            "hello-enrollment-authorization-required"
        );
    }

    #[test]
    fn statuses_follow_table() {
        for code in [
            ErrorCode::Unauthorized,
            ErrorCode::OwnerRequired,
            ErrorCode::ProxyRequired,
            ErrorCode::OriginRefused,
        ] {
            assert_eq!(code.http_status(), 403);
        }
        assert_eq!(ErrorCode::Locked.http_status(), 423);
        assert_eq!(ErrorCode::InvalidPatch.http_status(), 400);
        assert_eq!(ErrorCode::Engine("engine-timeout".into()).http_status(), 400);
    }

    #[test]
    fn engine_codes_are_validated() {
        assert_eq!(
            ErrorCode::engine("engine-timeout"),
            Ok(ErrorCode::Engine("engine-timeout".into()))
        );
        assert_eq!(
            ErrorCode::engine("sharing:forbidden").map(|c| c.to_string()),
            Ok("sharing:forbidden".into())
        );
        for bad in [
            "", "Engine-x", "sharing:", ":x", "a:b:c", "plain", "engine-1", "engine x",
        ] {
            assert_eq!(ErrorCode::engine(bad), Err(ErrorCode::RequestRefused), "{bad}");
        }
    }

    #[test]
    fn parse_maps_known_engine_and_unknown() {
        assert_eq!(ErrorCode::parse("locked"), ErrorCode::Locked);
        assert_eq!(
            ErrorCode::parse("sharing:forbidden"),
            ErrorCode::Engine("sharing:forbidden".into())
        );
        assert_eq!(ErrorCode::parse("Some Exception"), ErrorCode::RequestRefused);
    }

    #[test]
    fn error_trait_is_implemented() {
        let boxed: Box<dyn std::error::Error> = Box::new(ErrorCode::Locked);
        assert_eq!(boxed.to_string(), "locked");
    }
}
