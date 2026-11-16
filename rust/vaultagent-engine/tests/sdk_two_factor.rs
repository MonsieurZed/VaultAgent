//! Second facteur du moteur natif contre un Vaultwarden jetable : sans code, avec un mauvais code, avec le bon.
//! Le compte factice est inscrit puis équipé d'un TOTP par l'API Vaultwarden (secret factice), une seule fois.
//!
//! Variables : `VAULTAGENT_TEST_SERVER`, `VAULTAGENT_TEST_2FA_EMAIL`, `VAULTAGENT_TEST_2FA_PASSWORD` et, une fois le
//! TOTP activé (le premier lancement affiche le secret factice choisi par le serveur), `VAULTAGENT_TEST_2FA_SECRET`.
//! Lancement : `cargo test -p vaultagent-engine --test sdk_two_factor -- --ignored`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use bitwarden_core::Client;
use bitwarden_crypto::Kdf;
use serde_json::{Value, json};
use vaultagent_engine::{Account, Engine, EngineFactory, SdkEngineFactory};
use zeroize::Zeroizing;

/// Valeur d'une variable d'environnement obligatoire.
fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("variable {name} absente"))
}

/// Encode un texte pour un corps `application/x-www-form-urlencoded`.
fn form(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => char::from(b).to_string(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Code d'échec d'un résultat.
fn code<T>(result: vaultagent_core::Result<T>) -> String {
    result.err().map_or_else(String::new, |error| error.to_string())
}

/// Connexion d'un moteur neuf au compte à second facteur.
fn attempt(code_text: Option<&str>) -> vaultagent_core::Result<Box<dyn Engine>> {
    let server = env("VAULTAGENT_TEST_SERVER");
    let email = env("VAULTAGENT_TEST_2FA_EMAIL");
    let password = Zeroizing::new(env("VAULTAGENT_TEST_2FA_PASSWORD"));
    let supplied = code_text.map(|text| Zeroizing::new(text.to_owned()));
    let mut engine = SdkEngineFactory.start(Path::new("absent"), "test")?;
    engine.connect(
        &Account {
            server: &server,
            email: &email,
        },
        &password,
        supplied.as_ref(),
    )?;
    Ok(engine)
}

/// Code TOTP courant du secret factice.
fn current_code(secret: &str) -> String {
    bitwarden_vault::generate_totp(secret.to_owned(), None)
        .expect("code")
        .code
}

/// Inscrit le compte (sans effet s'il existe) et rend le hachage d'authentification du mot de passe maître.
async fn register(http: &reqwest::Client, server: &str, email: &str, password: &str) -> String {
    let keys = Client::new(None)
        .auth()
        .make_register_keys(email.to_owned(), password.to_owned(), Kdf::default_pbkdf2())
        .expect("clés");
    let hash = keys.master_password_hash.to_string();
    let token = http
        .post(format!("{server}/identity/accounts/register/send-verification-email"))
        .json(&json!({"email": email, "name": "factice", "receiveMarketingEmails": false}))
        .send()
        .await
        .expect("jeton d'inscription")
        .text()
        .await
        .expect("texte");
    let body = json!({
        "email": email, "masterPasswordHash": hash, "userSymmetricKey": keys.encrypted_user_key.to_string(),
        "userAsymmetricKeys": {"publicKey": keys.keys.public.to_string(), "encryptedPrivateKey": keys.keys.private.to_string()},
        "kdf": 0, "kdfIterations": 600_000, "emailVerificationToken": token.trim().trim_matches('"'),
    });
    http.post(format!("{server}/identity/accounts/register/finish"))
        .json(&body)
        .send()
        .await
        .expect("inscription");
    hash
}

/// Active le TOTP d'authentification du compte, avec le secret factice.
async fn enable(http: &reqwest::Client, server: &str, email: &str, hash: &str) -> String {
    let body = format!(
        "grant_type=password&username={}&password={}&scope=api%20offline_access&client_id=web&deviceType=9&deviceIdentifier=00000000-0000-4000-8000-000000000021&deviceName=test",
        form(email),
        form(hash)
    );
    let login: Value = http
        .post(format!("{server}/identity/connect/token"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("jeton")
        .json()
        .await
        .expect("json");
    let access = login["access_token"].as_str().expect("jeton d'accès").to_owned();
    let challenge: Value = http
        .post(format!("{server}/api/two-factor/get-authenticator"))
        .bearer_auth(&access)
        .json(&json!({"masterPasswordHash": hash}))
        .send()
        .await
        .expect("défi")
        .json()
        .await
        .expect("json du défi");
    let names: Vec<_> = challenge
        .as_object()
        .map_or_else(Vec::new, |o| o.keys().cloned().collect());
    let secret = challenge["key"]
        .as_str()
        .or(challenge["authenticator"]["key"].as_str())
        .unwrap_or_else(|| panic!("clé du serveur introuvable parmi {names:?}"))
        .to_owned();
    let verification = challenge["userVerificationToken"]
        .as_str()
        .expect("jeton de vérification");
    let response = http
        .put(format!("{server}/api/two-factor/authenticator"))
        .bearer_auth(&access)
        .json(&json!({"key": secret, "token": current_code(&secret), "userVerificationToken": verification}))
        .send()
        .await
        .expect("activation");
    let status = response.status();
    let text = response.text().await.expect("corps");
    assert!(status.is_success(), "activation du TOTP refusée : {status} {text}");
    eprintln!("secret factice du TOTP (à passer en VAULTAGENT_TEST_2FA_SECRET) : {secret}");
    secret
}

/// Prépare le compte : inscription puis activation du TOTP s'il n'est pas déjà exigé.
fn ensure_account() -> String {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let server = env("VAULTAGENT_TEST_SERVER");
    let email = env("VAULTAGENT_TEST_2FA_EMAIL");
    let password = env("VAULTAGENT_TEST_2FA_PASSWORD");
    let http = reqwest::Client::new();
    let hash = runtime.block_on(register(&http, &server, &email, &password));
    if code(attempt(None)) == "login:two-factor" {
        env("VAULTAGENT_TEST_2FA_SECRET")
    } else {
        runtime.block_on(enable(&http, &server, &email, &hash))
    }
}

#[test]
#[ignore = "exige un Vaultwarden de test"]
fn two_factor_without_code_with_a_wrong_code_and_with_the_right_code() {
    let secret = ensure_account();
    assert_eq!(code(attempt(None)), "login:two-factor");
    assert_eq!(code(attempt(Some("000000"))), "login:credentials");
    let mut engine = attempt(Some(&current_code(&secret))).expect("connexion avec le bon code");
    assert!(engine.items(None).is_ok());
    assert!(engine.folders().is_ok());
}
