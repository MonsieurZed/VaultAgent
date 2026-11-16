# Dépendances de l'espace de travail `rust/`

Une ligne de justification par dépendance directe, ajoutée dans la même tâche que le `Cargo.toml` qui l'utilise.
Les choix sont expliqués dans `docs/decisions/` (serveur HTTP, cryptographie, client HTTPS, exception `unsafe`).
La version exacte est celle de `Cargo.lock` ; la colonne Version donne la série retenue.

État à la tâche 2.4a : `vaultagent-server` utilise aussi `tiny_http` (série 0.12, sans fonction optionnelle ; ses dépendances `ascii`, `chunked_transfer`, `httpdate`, `log` suivent). État à la tâche 2.1c : `vaultagent-engine` utilise aussi les crates `bitwarden-*`, `tokio`, `reqwest` et `async-trait` (module `sdk`) ; licence de la crate : GPL-3.0-only. État à la tâche 2.2 : `vaultagent-server` utilise `serde`, `serde_json` et `zeroize` (plus `vaultagent-core` et `vaultagent-engine` par chemin) ; `getrandom` passe par `vaultagent_core::SystemEntropy`. État à la tâche 2.1 : `vaultagent-engine` utilise `serde`, `serde_json` et `zeroize` (plus `vaultagent-core` par chemin). État à la fin de la phase 1 : `vaultagent-core` utilise `serde`, `serde_json`, `hmac`, `sha2`, `rsa`, `p256`, `aes-gcm`, `subtle`, `getrandom`, `rand_core`, `zeroize` et `base64` ; les autres crates restent sans dépendance (`tiny_http`, `ureq`, `clap`, `windows-sys` arrivent avec leurs phases). Le tableau ci-dessous est la liste
de référence validée en tâche 0.1 ; une dépendance hors de cette liste exige une nouvelle note de décision.

| Crate | Version | Crates qui l'utiliseront | Fonctions activées | Justification |
|---|---|---|---|---|
| `serde` | 1.0 | toutes sauf `vaultagent-win32` | `derive`, `std` | Corps JSON typés du protocole et fichiers de `/data` ; standard de fait. |
| `serde_json` | 1.0 | toutes sauf `vaultagent-win32` | défaut, `float_roundtrip` | Lecture et écriture JSON (protocole, patch et sections opaques des fiches du moteur, magasin, sorties de la CLI). `float_roundtrip` lit les flottants comme Node, condition de l'empreinte d'idempotence identique. |
| `tiny_http` | 0.12 | `vaultagent-server` (transport, tâche 2.4a) | aucune | Serveur HTTP/1.1 synchrone derrière le proxy TLS ; donne l'adresse distante pour le contrôle du proxy de confiance. |
| `ureq` | 3 | `vaultagent-client` | `rustls`, `platform-verifier` | Client HTTPS synchrone, sans redirection, magasin de certificats du système. |
| `aes-gcm` | 0.10 | `vaultagent-core` | `aes`, `alloc` | AES-256-GCM du magasin (`protect`/`unprotect`) et de l'enveloppe `seal`, format identique à Node. |
| `rsa` | 0.9 | `vaultagent-core` | `std`, `sha2` | RSA-OAEP SHA-256 (`seal`), vérification RSA-PSS sel 32 et PKCS#1 v1.5 (WebAuthn RSA), clés SPKI DER. Avis Marvin non applicable au serveur (voir note cryptographie). |
| `p256` | 0.13 | `vaultagent-core` | `ecdsa`, `std` | Vérification ECDSA P-256 des preuves Windows Hello (WebAuthn EC2). |
| `hmac` | 0.12 | `vaultagent-core` | aucune | HMAC-SHA-256 des empreintes d'idempotence et de partage. |
| `sha2` | 0.10 | `vaultagent-core` | aucune | SHA-256 des empreintes, des défis WebAuthn et des signatures. |
| `subtle` | 2.6 | `vaultagent-core` | `std` | Comparaison à temps constant des jetons, codes et identifiants de clé. |
| `getrandom` | 0.2 | `vaultagent-core` (les autres crates passent par `Entropy` de core) | aucune | Aléa du système pour clés, nonces, défis, jetons et identifiants. Série 0.2 (et non 0.3) : celle que `rand_core` 0.6, donc `rsa` 0.9, tire déjà ; évite deux versions dans l'arbre. |
| `rand_core` | 0.6 | `vaultagent-core` | aucune | Traits `RngCore` et `CryptoRng` exigés par l'API OAEP de `rsa` ; la crate les implémente sur sa source d'aléa injectée (`EntropyRng`), `OsRng` n'est pas utilisé. |
| `zeroize` | 1 | `vaultagent-core`, `vaultagent-engine`, `vaultagent-server`, `vaultagent-client` | `std` | Effacement des secrets en clair à la destruction (`Zeroizing`), dont `Secret` du moteur et le mot de passe de `connect`. |
| `base64` | 0.22 | `vaultagent-core`, `vaultagent-client` | `std` | Base64 standard et `base64url` des clés, signatures, nonces et preuves. |
| `vaultagent-core` | chemin | `vaultagent-engine`, `vaultagent-server`, `vaultagent-client` | aucune | Crate de l'espace de travail (dépendance par chemin relatif) : `ErrorCode` et logique pure partagée. |
| `vaultagent-engine` | chemin | `vaultagent-server` | aucune | Crate de l'espace de travail : trait `Engine` typé, `EngineFactory`, types de domaine et faux moteur des tests du serveur. |
| `bitwarden-core` | git `bitwarden/sdk-internal`, `rev = 6238472451e4fa7b5c592b4b24a84e950a258957` (un seul commit pour toutes les crates `bitwarden-*`) | `vaultagent-engine` | `internal` | Client SDK officiel Bitwarden en natif, sous la branche GPL-3.0 de sa double licence (décision `2026-11-16-moteur-rust.md`) : connexion (`login_password`, 2FA), clés, jetons, organisations. Pas sur crates.io pour vault, sync et collections, d'où le git épinglé. |
| `bitwarden-crypto` | idem | `vaultagent-engine` | défaut | Clés d'organisation (`UnsignedSharedKey`) chargées à la synchronisation, KDF des tests. |
| `bitwarden-auth` | idem | `vaultagent-engine` | défaut | Gestionnaire de jetons du gestionnaire de mots de passe (`PasswordManagerTokenHandler`), qui renouvelle le jeton d'accès. |
| `bitwarden-vault` | idem | `vaultagent-engine` | défaut | Fiches (`ciphers().get_all`/`get`), dossiers (création, renommage), TOTP, déchiffrement des collections. |
| `bitwarden-sync` | idem | `vaultagent-engine` | défaut | `SyncClient` unique et trait `SyncHandler` du gestionnaire de fiches, d'organisations et de collections. |
| `bitwarden-state` | idem | `vaultagent-engine` | défaut | Dépôts en mémoire (`DatabaseConfiguration::Memory`) et trait `Repository` des fiches. |
| `bitwarden-collections` | idem | `vaultagent-engine` | défaut | Collections chiffrées reçues à la synchronisation et leur déchiffrement (`CollectionView`). |
| `bitwarden-api-api` | idem | `vaultagent-engine` | défaut | Modèles de la réponse de synchronisation, client d'API (suppression de dossier) et type d'erreur d'API classé par `sdk::errors`. |
| `bitwarden-pm` | idem | `vaultagent-engine` | défaut | Une seule fonction, `migrations::get_sdk_managed_migrations` : les migrations de dépôts (fiches, dossiers, valeurs persistées, clés d'organisation, Send, politiques, organisations) n'existent que là ; les réécrire exigerait `bitwarden-send`, `bitwarden-policies` et `bitwarden-organizations`, absentes de la liste. `bitwarden-encoding` n'a finalement pas été nécessaire (tâche 2.1d). |
| `tokio` | 1 | `vaultagent-engine` | `rt` | Runtime `current_thread` possédé par le moteur, derrière sa façade bloquante ; le SDK est async. Absent de `vaultagent-server`. |
| `reqwest` | 0.13 | `vaultagent-engine` | `rustls`, `json` | Client HTTP du SDK, livré sans TLS (`default-features = false`) : la crate active rustls, et lit la réponse du POST de création de collection (tâche 2.1d, requête envoyée par le client d'API du SDK) ; en 2.1c il ne sert qu'aux tests d'intégration (inscription du compte factice, activation de son TOTP) et au type d'erreur du SDK. |
| `async-trait` | 0.1 | `vaultagent-engine` | aucune | Implémentation du trait `SyncHandler` du SDK (handlers de fiches, organisations, collections). |
| `clap` | 4 | `vaultagent-client` (binaire) | `derive`, `std`, `help`, `usage`, `error-context` | Sous-commandes de la CLI `vaultagent`, aide et `--version`. |
| `windows-sys` | 0.61 | `vaultagent-win32` | modules Win32 utilisés seulement | Liaisons FFI brutes de Microsoft (fenêtres, `SendInput`, DPAPI, NCrypt, WebAuthn, barre). |
