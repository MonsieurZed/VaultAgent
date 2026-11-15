# Espace de travail Rust de VaultAgent

Réécriture en Rust de VaultAgent (plan : [`../docs/rust/plan.md`](../docs/rust/plan.md)). `vaultagent-core` contient les tâches 1.1 (écritures, projections) 1.2 (idempotence, vérification par relecture) et 1.3 (cryptographie, appareils, demandes d'association), 1.4 (Hello, demandes de déverrouillage) et 1.5 (coffres, partage) ; les autres crates sont vides à ce
stade. Dépendances : `serde`, `serde_json`, `hmac`, `sha2`, `rsa`, `p256`, `aes-gcm`, `subtle`, `getrandom`, `rand_core`, `zeroize` et `base64`. Les décisions d'architecture sont dans [`../docs/decisions/`](../docs/decisions/) ;
les dépendances prévues et leurs justifications dans [`DEPENDANCES.md`](DEPENDANCES.md).

| Crate | Rôle |
|---|---|
| `vaultagent-core` | Logique pure, sans fichier, réseau ni horloge. |
| `vaultagent-engine` | Processus enfant Node, protocole JSON ligne à ligne. |
| `vaultagent-server` | Magasin chiffré, session, routes d'API (Linux). |
| `vaultagent-client` | Binaire `vaultagent` : association, lecture, écriture, remplissage. |
| `vaultagent-win32` | Seule crate autorisée à utiliser `unsafe` (Windows uniquement), déclarée dans son `Cargo.toml`. |

## Commandes

Depuis `rust/` :

```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
conduite-check --crate rust/<crate> --outils
```

La dernière se lance depuis la racine du dépôt VaultAgent. Version de Rust fixée par `rust-toolchain.toml`.

## `vaultagent-core` : modules (tâches 1.1 à 1.5)

| Module | Rôle |
|---|---|
| `error` | `ErrorCode` (codes du protocole, `as_str`, `http_status`, `Engine(String)` validé par `ErrorCode::engine`), `Result<T>`. |
| `uuid`, `name`, `revision`, `https` | `Uuid`, `BoundedName`, `RevisionDate` (ISO, comparaison par instant), `https_origin` et `HttpsAddress`. |
| `canonical` | `canonical(&Value)` : clés triées (UTF-16), `null` retirés ; base de l'empreinte d'idempotence. |
| `request`, `new_item` | `WriteRequest::parse(&Value)` : règles de `validate` et `validateLifecycle`, `WriteAction`, `NewItem` ; `raw()` rend le JSON d'origine. |
| `patch`, `patch_uris` | `apply_write_patch(&mut Value, &RevisionDate, &Value, Option<&Uuid>)`, atomique, champs non ciblés conservés. |
| `projection` | `catalog_entry`, `item_metadata`, `item_detail` (sans secret, noms de champs du protocole), `visible`, `active`. |
| `canonical_json` | `canonical_json(&Value)` : octet pour octet `JSON.stringify(canonical(v))` de Node (clés UTF-16, indices d'abord, échappements, nombres JavaScript). Vecteurs Node : `tests/fixtures/canonical.json`. |
| `fingerprint` | `fingerprint(&[u8; 32], &Value)` : HMAC-SHA-256 hexadécimal minuscule de la forme canonique. |
| `ledger`, `ledger_flow` | `Receipt`, `LedgerEntry`, `Ledger`, `Registry` (`write-receipts.json`, `share-receipts.json`), trait `ReceiptStore` ; `open`, `replayed`, `ensure_capacity` (10 000), `record_intent`, `record_receipt`, `persist_*`. Format JSON identique à Node. |
| `verify` | `comparable`, `matches_created`, `verify_update` (historique des mots de passe), `ATTEMPTS` = 5, `RETRY_DELAY` = 200 ms ; la boucle d'essais vient avec le moteur (tâche 2.3). |
| `lifecycle`, `creation_checks` | `check_item_lifecycle`, `check_folder`, `confirm_lifecycle` (`Observed`), `check_create_duplicate`, `check_create_folder`, `check_folder_reference`, `confirm_folder_created`. |
| `jsval` | Sémantique JavaScript (vérité, longueur UTF-16, espaces) utilisée par les contrôles. |
| `entropy` | Trait `Entropy` (`fill`, `below` uniforme sans biais), `SystemEntropy` (`getrandom`), `SeededEntropy` (flux déterministe pour les tests), `EntropyRng` (adaptateur `RngCore` pour `rsa`). Échec : `engine-entropy`. |
| `encoding` | `digest` (SHA-256 hex), `same` (temps constant), `hex_encode`, base64 standard (lecture sans remplissage admise) et `base64url`. |
| `protect` | `protect`/`unprotect` AES-256-GCM, `Protected {nonce, tag, data}` identique à `store.mjs` ; toute faute de déchiffrement rend `invalid-key`. |
| `envelope` | `seal(valeur, spki_base64, entropy)` (RSA-OAEP SHA-256 + AES-GCM, clé RSA de 2048 à 4096 bits) et `open_envelope(enveloppe, clé_privée, entropy)`, `Envelope {encryptedKey, nonce, tag, data}`. |
| `approver_key` | `rsa_public_key` (RSA 2048 exactement, 1024 caractères base64 au plus), `verify_pss` (RSA-PSS SHA-256, sel 32, code d'échec au choix), `decision_message` (tableau `VaultAgent.Unlock.v2`). |
| `device_record`, `device_token` | `DeviceRecord` (format exact de `devices.json`, champs inconnus conservés), `PublicDevice`, `HelloCredential`, `iso_from_millis` ; jeton `va1.<uuid>.<base64url>` (`issue_token`, `parse_bearer`, `random_uuid`). |
| `devices`, `device_list` | `Devices` (codes d'association : `issue`, `pair`, limites 8 codes et 10 essais par minute), `enroll`, `enroll_named` ; `authenticate`, `list`, `revoke`, `set_automatic`, `clear_automatic`, `authorize_hello_enrollment`, `enroll_hello`, `enroll_elevation`. Les mutations rendent la nouvelle liste, l'écriture du fichier viendra avec la tâche 2.2. |
| `pair_requests`, `pair_types` | `PairRequests` : `open`, `list`, `target`, `approve` (prend le résultat de l'enrôlement), `deny`, `poll` (jeton remis une fois) ; réponses `Opened`, `Listed`, `Polled`. |
| `hello` | `verify_hello(Option<&HelloCredential>, Option<&HelloProof>, message)` (EC2 P-256 DER ou RSA PKCS#1 v1.5, `hello-required` si preuve absente, `hello-invalid` sinon), `hello_public_key`, `hello_challenge`, `HelloProof`, `HELLO_RP_ID`, `HELLO_ORIGIN`. Vecteurs Node : `tests/fixtures/unlock.json`. |
| `session_control`, `approval_policy` | Trait `SessionControl` (`unlocked`, `generation`, `profile`, `unlock(code)`, `lock`) abstrait la session ; `ApprovalPolicy` (`approval-policy.json`, `requires_hello_for`) est passée en paramètre, le fichier se lit et s'écrit en tâche 2.2. |
| `unlock_requests`, `unlock_contexts`, `unlock_decision` | `UnlockRequests::new(vault_id)` : `current`, `request` (acceptation automatique), `list`, `decide`, `context` (`reset-auto`, 60 s), `security`, `begin_hello`/`complete_hello_enrollment`, `begin_elevation`/`complete_elevation_enrollment`, `signed_decision` (RSA-PSS, Hello, élévation, `remember`, `reset-auto`), `status`. Tout passe par `Env` (session, aléa, heure en ms, appareils, politique). Les mutations d'appareils et de politique sont rendues dans `Outcome {value, devices, policy}` pour que l'appelant les enregistre. |
| `unlock_replies`, `unlock_inputs` | Formes JSON publiques (`PublicRequest`, `UnlockReply`, `DecisionReply`, `ApprovalSecurity`, `UnlockStatus`…) et corps reçus (`SignedDecisionInput`, `HelloEnrollInput`, `ElevationEnrollInput`). |
| `vault_registry` | `VaultRegistry` au format de `vaults.json` (`[{id, name}]`, défaut `Coffre existant`) : `init(Option<&Value>)` (1 à 8 coffres, ids uniques, `default` présent, UUID sinon : `invalid-vault-registry`, `invalid-vault`), `save_name(id, nom, aléa)` (nom 80, `invalid-vault-name`, `vault-limit`, `vault-missing`) rend `SavedVault {registry, entry, created}` sans rien écrire, `to_json`. |
| `vault_select` | `VaultDevices {id, profile, records}` ; `authorized` (réutilise `device_list::authenticate`), `select` (`unauthorized`, `vault-required`), `VaultMetadata` (`{id, name, …statut}` de `/api/agent/vaults`, statut fourni par l'appelant). |
| `vault_clients` | `ensure_unique_account` (`duplicate-account` : même origine, courriel insensible à la casse), `available_clients`, `grant` (copie l'appareil vers un autre coffre, `autoUnlock` faux, `not-configured`, `invalid-device`, `device-limit`) rend `Grant {records, changed}`. |
| `share_request`, `share_checks` | `ShareRequest::parse` (`invalid-share`, actions `preview`, `apply`, `create-collection`, bornes) ; `find_organization`, `check_item`, `check_confirmation`, `check_new_collection`, `preview` (`SharePreview`). |
| `share_verify`, `share_flow` | `item_moved` (organisation, collections triées, 10 champs en forme canonique), `collection_created`, `receipt` ; `begin` ouvre le registre `Registry::Share` (empreinte de la requête entière) : `ShareStart::{Preview, Replayed, Fresh}`, puis `ledger_flow` pour la capacité, l'intention et le reçu. |

Usage : `let request = WriteRequest::parse(&body)?;` puis `match request.action()`. Les tests d'intégration
(`tests/`) rejouent aussi des cas dorés de `contrat/`.
