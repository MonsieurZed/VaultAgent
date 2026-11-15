# Dépendances de l'espace de travail `rust/`

Une ligne de justification par dépendance directe, ajoutée dans la même tâche que le `Cargo.toml` qui l'utilise.
Les choix sont expliqués dans `docs/decisions/` (serveur HTTP, cryptographie, client HTTPS, exception `unsafe`).
La version exacte est celle de `Cargo.lock` ; la colonne Version donne la série retenue.

État à la fin de la phase 0 : **aucune dépendance**, les crates sont vides. Le tableau ci-dessous est la liste
de référence validée en tâche 0.1 ; une dépendance hors de cette liste exige une nouvelle note de décision.

| Crate | Version | Crates qui l'utiliseront | Fonctions activées | Justification |
|---|---|---|---|---|
| `serde` | 1.0 | toutes sauf `vaultagent-win32` | `derive`, `std` | Corps JSON typés du protocole et fichiers de `/data` ; standard de fait. |
| `serde_json` | 1.0 | toutes sauf `vaultagent-win32` | défaut | Lecture et écriture JSON (protocole, moteur ligne à ligne, magasin, sorties de la CLI). |
| `tiny_http` | 0.12 | `vaultagent-server` | aucune | Serveur HTTP/1.1 synchrone derrière le proxy TLS ; donne l'adresse distante pour le contrôle du proxy de confiance. |
| `ureq` | 3 | `vaultagent-client` | `rustls`, `platform-verifier` | Client HTTPS synchrone, sans redirection, magasin de certificats du système. |
| `aes-gcm` | 0.10 | `vaultagent-core` | `aes`, `alloc` | AES-256-GCM du magasin (`protect`/`unprotect`) et de l'enveloppe `seal`, format identique à Node. |
| `rsa` | 0.9 | `vaultagent-core` | `std`, `sha2` | RSA-OAEP SHA-256 (`seal`), vérification RSA-PSS sel 32 et PKCS#1 v1.5 (WebAuthn RSA), clés SPKI DER. Avis Marvin non applicable au serveur (voir note cryptographie). |
| `p256` | 0.13 | `vaultagent-core` | `ecdsa`, `std` | Vérification ECDSA P-256 des preuves Windows Hello (WebAuthn EC2). |
| `hmac` | 0.12 | `vaultagent-core` | aucune | HMAC-SHA-256 des empreintes d'idempotence et de partage. |
| `sha2` | 0.10 | `vaultagent-core` | aucune | SHA-256 des empreintes, des défis WebAuthn et des signatures. |
| `subtle` | 2.6 | `vaultagent-core` | `std` | Comparaison à temps constant des jetons, codes et identifiants de clé. |
| `getrandom` | 0.3 | `vaultagent-core`, `vaultagent-server`, `vaultagent-client` | aucune | Aléa du système pour clés, nonces, défis, jetons et identifiants. |
| `rand_core` | 0.6 | `vaultagent-core` | `getrandom` | Générateur `OsRng` exigé par l'API de chiffrement OAEP de `rsa`. |
| `zeroize` | 1 | `vaultagent-core`, `vaultagent-client` | `std` | Effacement des secrets en clair à la destruction (`Zeroizing`). |
| `base64` | 0.22 | `vaultagent-core`, `vaultagent-client` | `std` | Base64 standard et `base64url` des clés, signatures, nonces et preuves. |
| `clap` | 4 | `vaultagent-client` (binaire) | `derive`, `std`, `help`, `usage`, `error-context` | Sous-commandes de la CLI `vaultagent`, aide et `--version`. |
| `windows-sys` | 0.61 | `vaultagent-win32` | modules Win32 utilisés seulement | Liaisons FFI brutes de Microsoft (fenêtres, `SendInput`, DPAPI, NCrypt, WebAuthn, barre). |
