//! Connexion d'un compte : prélogin, jeton (avec le second facteur TOTP si fourni), clés, puis première synchro.

use bitwarden_core::auth::login::{PasswordLoginRequest, TwoFactorProvider, TwoFactorRequest};
use vaultagent_core::Result;
use zeroize::Zeroizing;

use crate::engine::{Account, stage_error};
use crate::sdk::errors::login_error;
use crate::sdk::session::Session;

/// Ouvre la session d'un compte.
///
/// Paramètres : le compte, le mot de passe maître et le code TOTP facultatif.
/// Retour : la session connectée, coffre synchronisé.
///
/// # Errors
///
/// `login:two-factor` si le serveur demande un second facteur que le code ne fournit pas, `login:credentials`
/// si le serveur refuse le mot de passe ou le code, `login:network`, `login:tls`, `login:rejected`, ou
/// `sync:*` si la première synchronisation échoue.
pub async fn connect(
    account: &Account<'_>,
    password: &Zeroizing<String>,
    code: Option<&Zeroizing<String>>,
) -> Result<Session> {
    let session = Session::build(account.server).await?;
    let request = PasswordLoginRequest {
        email: account.email.to_owned(),
        password: password.as_str().to_owned(),
        two_factor: code.map(|token| TwoFactorRequest {
            token: token.as_str().to_owned(),
            provider: TwoFactorProvider::Authenticator,
            remember: false,
        }),
    };
    let response = session
        .client
        .auth()
        .login_password(&request)
        .await
        .map_err(|e| login_error(&e))?;
    if !response.authenticated {
        return Err(stage_error("login", "two-factor"));
    }
    session.init_user().await?;
    session.resync().await?;
    Ok(session)
}
