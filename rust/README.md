# Espace de travail Rust de VaultAgent

Réécriture en Rust de VaultAgent (plan : [`../docs/rust/plan.md`](../docs/rust/plan.md)). `vaultagent-core` contient les tâches 1.1 (écritures, projections) 1.2 (idempotence, vérification par relecture) et 1.3 (cryptographie, appareils, demandes d'association), 1.4 (Hello, demandes de déverrouillage) et 1.5 (coffres, partage) ; `vaultagent-engine` contient la tâche 2.1b ; `vaultagent-server` contient les tâches 2.2, 2.3 (bibliothèque) 2.4a (binaire, administration) et 2.4b (front HTML sans JavaScript) ; les autres crates sont vides à ce
stade. Dépendances : `serde`, `serde_json`, `hmac`, `sha2`, `rsa`, `p256`, `aes-gcm`, `subtle`, `getrandom`, `rand_core`, `zeroize` et `base64`. Les décisions d'architecture sont dans [`../docs/decisions/`](../docs/decisions/) ;
les dépendances prévues et leurs justifications dans [`DEPENDANCES.md`](DEPENDANCES.md).

| Crate | Rôle |
|---|---|
| `vaultagent-core` | Logique pure, sans fichier, réseau ni horloge. |
| `vaultagent-engine` | Trait `Engine` typé, types de domaine, faux moteur, moteur natif `sdk` sur le SDK Bitwarden (connexion et lectures en 2.1c, écritures en 2.1d). Licence GPL-3.0-only. |
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

## `vaultagent-engine` : modules (tâche 2.1b)

Dépend de `vaultagent-core` (`ErrorCode`, `NewItem`, `RevisionDate`, `Uuid`, `apply_write_patch`), `serde`, `serde_json`, `zeroize`. Le pilote
du processus Node (`fork/agent.ts`) est retiré : le moteur natif sur le SDK Bitwarden (2.1c et 2.1d) vient derrière
le trait ci-dessous, bloquant (la façade `async` y restera confinée).

| Module | Rôle |
|---|---|
| `engine` | Trait `Engine` : une méthode par opération (`connect`, `lock`, `sync`, `items`, `trashed_items`, `item`, `totp`, `folders`, `create_folder`, `rename_folder`, `delete_folder`, `create_item`, `update_item` (révision attendue, patch de `vaultagent-core`), `trash_item`, `restore_item`, `organizations`, `check_sharing`, `create_collection`, `share_item`, `passkeys`, `create_passkey`, `alive`, `kill`). `EngineFactory::start(directory, profile)`, `Account`, `fail(code)` pour les `engine-*`, `stage_error(étape, catégorie)` pour `login:credentials`, `write:rejected`, `sharing:refused`. |
| `item` | `Item`, `Login`, `LoginUri`, `PasswordHistoryEntry` : les champs lus par `linux/*.mjs` et `vaultagent-core`, noms Bitwarden en camelCase ; les sections `secure_note`, `card`, `identity`, `ssh_key`, `fields` ne servent qu'à comparer avant et après un partage. |
| `structure` | `Folder`, `Organization` (`role`), `Collection` (`can_edit`). |
| `passkey` | `Passkey` (identifiant FIDO2 complet) et `PasskeyEntry` (fiche porteuse). |
| `secret` | `Secret` : texte effacé à la destruction, `Debug` n'écrit que la longueur. Mot de passe, TOTP, notes et clé de passkey. |
| `fake`, `fake_engine`, `fake_call`, `fake_reply` | `FakeEngine`, `FakeHandle` (`push`, `push_ok`, `push_error`, `calls`, `call_names`, `kills`, `set_alive`), `FakeFactory`, `Call` (journal typé), `Reply` (réponse scriptée d'une forme donnée, `engine-refused` si elle ne convient pas à la méthode). Toujours compilé pour les tests du serveur (2.2, 2.3). |

Le mot de passe de `connect` reste un `Zeroizing<String>` côté serveur. Les tests sont `tests/fake.rs` et les modules
`#[cfg(test)]` de `engine`, `item`, `structure` et `secret`.

## `vaultagent-engine::sdk` : moteur natif sur le SDK Bitwarden (tâches 2.1c et 2.1d)

Licence : `vaultagent-engine` est en `GPL-3.0-only` (et non MIT comme le reste de l'espace de travail), car elle lie
`sdk-internal` de Bitwarden sous sa branche GPL-3.0 ; le dépôt entier le devient à la distribution du binaire.
`SdkEngineFactory::start(dossier, profil)` rend un `SdkEngine` non connecté ; chaque moteur possède un runtime tokio
`current_thread` et appelle `block_on` par méthode, le trait reste bloquant et `vaultagent-server` ne dépend pas de
tokio. L'état du SDK est en mémoire (`DatabaseConfiguration::Memory`) : le dossier de données n'est ni lu ni écrit.
`lock` jette le `Client` SDK (clés, dépôts, jetons) ; les lectures rendent alors `locked`, et `engine-locked` après `kill`.

| Module | Rôle |
|---|---|
| `sdk::engine`, `sdk::factory` | `SdkEngine` (trait `Engine`) et `SdkEngineFactory` ; chaque méthode délègue à un module du dossier `sdk`. |
| `sdk::write`, `sdk::patch`, `sdk::write_errors` | Écritures de fiches (2.1d) : `create_item` (connexion ou note, dossier, organisation et collections facultatifs), `update_item` (resynchronise, relit, `apply_write_patch` de `vaultagent-core` sur le `CipherView` en JSON, puis `ciphers().edit` avec la `revision_date` relue ; révision différente ou 400 « out of date » de Vaultwarden = `revision-conflict` ; sans droit d'édition ou fiche personnelle sous la politique `OrganizationDataOwnership` = `write:rejected`), `trash_item`, `restore_item`. |
| `sdk::share`, `sdk::passkeys`, `sdk::clock` | `create_collection` (nom chiffré par le SDK, POST sur `/organizations/{id}/collections` par le client d'API du SDK avec le jeton de la session, puis resynchronisation), `share_item` (`check_sharing`, `share_cipher`, resynchronisation) ; `passkeys` (lecture de `login.fido2Credentials`, filtre `rpId`) et `create_passkey` (fiche vers `https://<rpId>`, relue, `passkeys:passkey-storage` si `credentialId`, `keyValue` ou `rpId` diffèrent) par le chemin natif, sans la fonctionnalité `wasm` ; `clock` : date ISO du jour. |
| `sdk::session`, `sdk::login` | `Session` (client, `SyncClient` unique, état des organisations, `init_user_id` depuis le jeton, `resync`) ; connexion `login_password` avec 2FA TOTP (`TwoFactorRequest`, fournisseur authentificateur) : `login:two-factor` sans code, `login:credentials` si le code ou le mot de passe est refusé. |
| `sdk::sync_handler` | Gestionnaire de sync des fiches, des organisations actives (`enabled`, statut confirmé), des collections et de la politique `OrganizationDataOwnership` (`data_ownership`), clés d'organisation chargées. |
| `sdk::read`, `sdk::filter`, `sdk::convert` | Liste, corbeille, lecture, TOTP ; filtre (types 1 et 2, sans `archivedDate`) et recherche (nom, identifiant, notes, adresses) ; conversion `CipherView` vers `Item` (dates ISO UTC). |
| `sdk::folders`, `sdk::org` | Dossiers (liste, création, renommage, suppression par l'API puis resynchronisation) ; organisations avec collections déchiffrées, `check_sharing` (`sharing:refused`). Les collections qui ne se déchiffrent pas sont omises. |
| `sdk::errors` | Classement par type en `<étape>:<catégorie>` : `two-factor`, `credentials`, `tls`, `network`, `rejected` (statut 400 de l'identité = `credentials`, 429 et 5xx = `network`). Aucun message du SDK dans le code rendu. |

Tests : modules `#[cfg(test)]` (classification, filtre, conversion, patch, passkeys, purge au `lock`, écritures verrouillées) sans
réseau ; `tests/sdk_integration.rs`, `tests/sdk_writes.rs` et `tests/sdk_two_factor.rs` sont `#[ignore]` et visent un Vaultwarden jetable :

```
VAULTAGENT_TEST_SERVER=http://localhost:8380 VAULTAGENT_TEST_EMAIL=… VAULTAGENT_TEST_PASSWORD=… \
  cargo test -p vaultagent-engine --test sdk_integration -- --ignored
  cargo test -p vaultagent-engine --test sdk_writes -- --ignored
VAULTAGENT_TEST_2FA_EMAIL=… VAULTAGENT_TEST_2FA_PASSWORD=… [VAULTAGENT_TEST_2FA_SECRET=…] \
  cargo test -p vaultagent-engine --test sdk_two_factor -- --ignored
```

Vaultwarden limite les connexions (rafale de 10, puis une par minute) : un lancement en consomme de quatre à six,
redémarrer le conteneur jetable entre deux lancements. Le premier lancement du test 2FA inscrit le compte, active son
TOTP et affiche le secret factice à passer ensuite en `VAULTAGENT_TEST_2FA_SECRET`.

## `vaultagent-server` : modules (tâches 2.2 et 2.3, bibliothèque)

Dépend de `vaultagent-core`, `vaultagent-engine`, `serde`, `serde_json`, `zeroize`. Le binaire, le transport (`tiny_http`),
`/api/pair`, `/api/admin/*`, le front et les téléchargements : voir plus bas (tâches 2.4a et 2.4b) ; avant 2.4 ils rendaient
`owner-required` sans le bon `x-authentik-username`, sinon `route-refused`.

Usage (2.3) : `route(&Request, &Context) -> Response` est le cœur HTTP pur. `Context` porte les coffres, l'origine, le proxy de
confiance, le propriétaire, l'horloge (`Clock`), l'aléa (`SharedEntropy`) et l'attente (`Pause`, `ThreadPause` en production,
`RecordedPause` en test). Ordre des contrôles : `GET /healthz`, proxy, origine, puis chemin ; sous `/api/agent/` : jeton,
méthode `POST`, `content-type` JSON, taille (24 576 octets), JSON, choix du coffre, route.

| Module | Rôle |
|---|---|
| `error` | `ServerError` : `Code(ErrorCode)` plus les codes propres au serveur (`private-key-permissions`, `private-key-invalid`, `invalid-account`, `invalid-timeout`, `password-required`, `cancelled`) ; `code_text`, `into_code`. |
| `store`, `store_files` | `Store::open(dossier)` : dossier 0700, `machine.key` en exclusif 0600 (32 octets, mode contrôlé sous Unix), `load`/`load_optional`, `save` atomique (`<nom>.<16 hex>.tmp` puis `rename`), `protect`/`unprotect`, `account`, `devices`, `update_devices` (sérialisé), `approval_policy`, et `ReceiptStore` pour `&Store` (`write-receipts.json`, `share-receipts.json`). Format de `/data` identique à Node. |
| `private_fs` | Seul endroit des appels propres à Unix (`cfg(unix)`, sans `unsafe`) : dossier 0700, fichier neuf 0600, contrôle du mode de la clé. |
| `account` | `Account` : format de `account.json` (`server`, `email`, `timeoutMinutes`, `profile`, `password: {nonce, tag, data}`). |
| `clock` | Trait `Clock` (`monotonic_ms`, `wall_ms`), `SystemClock`, `ManualClock` (tests). |
| `shared_entropy` | `SharedEntropy` : source d'aléa partageable (`system()` ou déterministe en test). |
| `session`, `session_configure`, `configure_input`, `engine_slot` | `Session` : `status` (ne prolonge pas le délai), `touch`, `lock` (génération +1, moteur condamné), `expire`, `access(generation)` (`locked`), `serial()` (verrou des opérations ; `lock` ne le prend pas, donc il invalide une opération en cours), `configure(&ConfigureInput)`, `unlock(code)` ; `SessionControl` implémenté pour `&Session`. `EngineSlot::run(|engine| …)` refuse (`locked`) un moteur condamné et l'arrête à la fin d'une requête en cours. |
| `vaults` | `Vaults::open` (`vaults.json` validé par `VaultRegistry`), un `VaultContext` par coffre (`store`, `session`, `devices`, `approvals`), `get`, `metadata`, `save_name`, `configure` (`duplicate-account`), `expire`, `lock`. |

Les tests (`tests/store.rs`, `session.rs`, `vaults.rs`, `node_data.rs`) utilisent des dossiers temporaires nettoyés,
`FakeFactory` et `ManualClock` ; `tests/fixtures/node-data/` contient un `/data` produit par Node (clé factice).

| Module (2.3) | Rôle |
|---|---|
| `http` | `Request` (méthode, chemin, en-têtes, adresse distante, corps) et `Response` (`json`, `error` : table 403, 423, 400 et repli `request-refused`). |
| `route`, `context`, `pause` | Cœur HTTP pur, contexte injecté, attente injectée. |
| `agent`, `agent_auth`, `guard` | Aiguillage des routes agent ; coffres autorisés par jeton, `vaultId` ; `Guard` = `authorize(); session.access(generation)` de Node (relève la génération, contrôle le client et la génération après chaque appel du moteur). |
| `agent_status`, `agent_read`, `agent_seal` | `vaults`, `status`, `lock` ; `catalog`, `write-preview`, `sharing` (resynchronisation avant lecture) ; `seal` (attente du TOTP si moins de 5 s restent). |
| `agent_write`, `agent_lifecycle`, `agent_share`, `write_ledger`, `verify_loop` | `write` (création, mise à jour, dossiers, corbeille, restauration), `share` ; registre d'idempotence, reçus ; relecture 5 fois toutes les 200 ms (attente injectée). |
| `agent_unlock` | `request-unlock`, `decide-unlock`, `approval-context`, `hello-*`, `elevation-*` (environnement de `UnlockRequests` : session, aléa, heure, appareils, politique ; appareils et politique modifiés sont enregistrés). |
| `item_value` | Adaptateur unique `Item`/`Folder`/`Organization` vers les `Value` lues par les projections de `vaultagent-core` (les projections de core restent sur `Value`) : `attachments` devient un tableau quand `has_attachments`. |

Tests 2.3 : `tests/golden.rs` rejoue les cas de `rust/contrat/` (`--nocapture` affiche les comptes) ; `routes_http`, `routes_read`,
`routes_seal`, `routes_write`, `routes_lifecycle`, `routes_share`, `routes_vaults` suivent l'intention des tests Node. L'aide
`tests/common/service.rs` monte un service complet (coffres, faux moteur, horloge et attente factices, client associé).

## `vaultagent-server` : binaire, administration et transport (tâche 2.4a)

Dépend aussi de `tiny_http` 0.12 (sans fonction optionnelle). Le front est rendu par Rust (tâche 2.4b, section suivante) ;
`linux/web/` ne sert plus qu'au serveur Node, jusqu'à la tâche 5.2.

Lancement, depuis `rust/` (le serveur écoute en HTTP clair derrière le proxy TLS ; l'arrêt par SIGTERM tue le processus, rien
n'est à nettoyer : moteur dans le processus, écritures atomiques) :

```
VAULTAGENT_ORIGIN=https://vaultagent.exemple VAULTAGENT_TRUSTED_PROXY=10.0.0.2 VAULTAGENT_OWNER=nom-authentik \
cargo run -p vaultagent-server
```

| Variable | Rôle |
|---|---|
| `VAULTAGENT_ORIGIN` | Obligatoire : origine HTTPS exacte du service (sans chemin ni `/` final). |
| `VAULTAGENT_TRUSTED_PROXY` | Obligatoire : adresse IP du proxy de confiance (seule adresse admise hors `GET /healthz`). |
| `VAULTAGENT_OWNER` | Obligatoire : valeur attendue de `x-authentik-username`. |
| `VAULTAGENT_DATA` | Dossier des données, `/data` par défaut. |
| `VAULTAGENT_DOWNLOADS` | Dossier des archives de `/downloads/*` ; absent, la route tombe dans `origin-refused` comme Node. |
| `VAULTAGENT_BIND`, `VAULTAGENT_PORT` | Adresse et port d'écoute, `0.0.0.0` et `8080` par défaut (en dur dans Node). |
| `VAULTAGENT_WORKERS` | Fils de travail, 8 par défaut, de 1 à 64. |

| Module (2.4a) | Rôle |
|---|---|
| `owner` | Aiguillage du propriétaire : fichiers, `GET /` (page), `GET /api/admin/state`, `POST /admin/*` (formulaires), `404` pour `/app.js` et `/index.html`, puis contrôle d'origine de l'API JSON (`POST` et en-tête `origin` exacts) et `admin`. |
| `admin`, `admin_devices`, `admin_state` | `admin_state` : `snapshot` (état construit une fois, pour le JSON et le HTML) et `state` ; `admin::act` : l'action d'un corps JSON déjà lu, appelée par la route JSON et par les formulaires ; `write-preview`, `write` (garde `Guard::owner`, sans jeton), `configure`, `unlock`, `decide-unlock`, `lock` ; `auto-unlock`, `pairing`, `pair-decide`, `revoke`. `vaultId` absent = coffre `default`, non chaîne = `vault-missing`. |
| `pair_route` | `POST /api/pair` : `request`, `poll`, code `<coffre>.<code>`. Les demandes en attente sont communes à tous les coffres (`Context::pair_requests`). |
| `files`, `query` | Archives (liste fermée de quatre noms, lues en mémoire, `404 download-unavailable`) et `GET /style.css` ; lecture de `?vaultId=` (décodage `+` et `%XX`). |
| `settings`, `bootstrap` | `Settings::from_lookup` (variables ci-dessus) ; `build_context(&Settings, fabrique de moteurs)`. |
| `transport`, `transport_request`, `transport_response` | `serve(adresse, fils, &Context) -> Running` : `tiny_http`, un ensemble fixe de fils qui appellent `recv_timeout`, minuterie d'expiration toutes les 500 ms ; en-têtes limités à 40 (431), corps lu jusqu'à 24 577 octets. `Running::address`, `shutdown`, `wait`. |
| `main` (binaire) | Environnement, `SdkEngineFactory`, `serve`, `wait`. |

Tests 2.4a : `routes_admin` et `routes_files` (sans socket), `transport` (vrais sockets sur `127.0.0.1:0`, faux moteur, client `TcpStream` :
parcours configure, code, association, statut agent, front ; proxy ; limites ; connexion inactive). `tests/golden.rs` rejoue les 49 cas.

## `vaultagent-server` : front HTML sans JavaScript (tâche 2.4b)

Aucune dépendance nouvelle ni variable d'environnement (`VAULTAGENT_WEB` est supprimée : la feuille de style est dans le
binaire par `include_str!`, `assets/style.css`). Le front de `linux/web/` n'est plus servi : `/app.js` et `/index.html` rendent `404`.

- **`GET /`** : page complète rendue depuis l'état du coffre `default` (même donnée que `GET /api/admin/state`), pour le
  propriétaire Authentik derrière le proxy de confiance. Onglets : liens `?tab=account|clients|about`, seul l'actif est rendu.
  Messages : `?msg=saved|refused|login-refused|duplicate-account|expired`, des codes traduits au rendu.
- **Langue** : `?lang=fr|en|es` pose le cookie `vaultagent-language` (`Path=/; SameSite=Strict; HttpOnly; Secure`) ; français par défaut.
- **Sécurité** : aucun `<script>` ni attribut `on*` ; `Content-Security-Policy: default-src 'none'; style-src 'self'; img-src 'self';
  form-action 'self'; frame-ancestors 'none'; base-uri 'none'`, plus `no-store`, `nosniff`, `no-referrer`. Toute donnée insérée passe par
  `front_escape::escape`. Mot de passe et codes 2FA : jamais renvoyés (champs vides), jamais dans une URL.
- **Actions** : formulaires `POST /admin/<configure|unlock|lock|pairing|pair-decide|decide-unlock|auto-unlock|revoke>` en
  `application/x-www-form-urlencoded`. Le propriétaire est contrôlé par `route`, l'en-tête `origin` doit valoir `VAULTAGENT_ORIGIN`
  (sinon `403 origin-refused`), puis `admin::act` (la même fonction que la route JSON) est appelée et la réponse est un `303` vers
  `/?tab=…&msg=<code>`. `pairing` répond directement une page `200` `no-store` (code dans un `<details>`) : le code n'entre dans aucune URL.
- **Rafraîchissement** : `<meta http-equiv="refresh" content="5">` sur « Clients » tant qu'aucun message n'est affiché, et sur tout onglet sauf
  « Compte » quand des demandes attendent ; jamais sur « Compte » (une saisie serait perdue). Lien « Actualiser » partout.
- **Dates** : rendues côté serveur en UTC (`HH:MM UTC`, avec la date pour les appareils) ; la bibliothèque standard n'a pas de fuseau.

| Module (2.4b) | Rôle |
|---|---|
| `front_page`, `front_view` | `GET /` : choix de langue, d'onglet et de message, puis assemblage de la page (rafraîchissement, liste de langues, onglets, pied de page). |
| `front_status`, `front_account`, `front_clients`, `front_devices`, `front_about` | Organismes de la page : statut et demandes de déverrouillage, formulaire du compte, onglet Clients (guide, demandes d'association, création du code), appareils, « Fonctionnement ». |
| `front_html`, `front_escape` | `View` (contexte de rendu), champs cachés, balise de formulaire ; `escape` (cinq caractères). |
| `front_action`, `front_body`, `form` | `POST /admin/*` : contrôles, `Form::parse` (analyseur `x-www-form-urlencoded`, std seulement), corps JSON de l'action, redirection. |
| `front_pairing`, `front_response`, `front_notice`, `front_tab`, `front_lang`, `front_dates` | Page du code ; réponses (CSP, redirection 303, `style.css`) ; messages ; onglets ; langue et cookie ; dates UTC. |
| `i18n`, `i18n_layout`, `i18n_account`, `i18n_clients` | Tables fr/en/es indexées par le texte français (71 entrées : les 64 de l'ancien `app.js` et 7 nouvelles) ; un texte absent reste en français. |

Tests 2.4b : `front_pages` (onglets en trois langues, échappement, champs vides, aucun script, rafraîchissement, cookie), `front_actions`
(origine, propriétaire, corps, chaque action, page du code), tests unitaires de chaque module, `transport` (`GET /` sur socket réel).
Le cas doré `front-index` décrit la nouvelle page (CSP sans script) ; les 48 autres sont inchangés.
