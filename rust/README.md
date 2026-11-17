# Espace de travail Rust de VaultAgent

Réécriture en Rust de VaultAgent (plan : [`../docs/rust/plan.md`](../docs/rust/plan.md)). `vaultagent-core` contient les tâches 1.1 (écritures, projections) 1.2 (idempotence, vérification par relecture) et 1.3 (cryptographie, appareils, demandes d'association), 1.4 (Hello, demandes de déverrouillage) et 1.5 (coffres, partage) ; `vaultagent-engine` contient la tâche 2.1b ; `vaultagent-server` contient les tâches 2.2, 2.3 (bibliothèque) 2.4a (binaire, administration) et 2.4b (front HTML sans JavaScript) ; les autres crates sont vides à ce
stade. Dépendances : `serde`, `serde_json`, `hmac`, `sha2`, `rsa`, `p256`, `aes-gcm`, `subtle`, `getrandom`, `rand_core`, `zeroize` et `base64`. Les décisions d'architecture sont dans [`../docs/decisions/`](../docs/decisions/) ;
les dépendances prévues et leurs justifications dans [`DEPENDANCES.md`](DEPENDANCES.md).

| Crate | Rôle |
|---|---|
| `vaultagent-core` | Logique pure, sans fichier, réseau ni horloge. |
| `vaultagent-engine` | Trait `Engine` typé, types de domaine, faux moteur, moteur natif `sdk` sur le SDK Bitwarden (connexion et lectures en 2.1c, écritures en 2.1d). Licence GPL-3.0-only. |
| `vaultagent-server` | Magasin chiffré, session, routes d'API (Linux). |
| `vaultagent-client` | Binaire `vaultagent` : association, lecture, écriture, `seal`, remplissage ; commandes de fenêtre `foreground`, `focus`, `type` sous Windows (4.1). |
| `vaultagent-win32` | Seule crate autorisée à utiliser `unsafe` (Windows uniquement, vide ailleurs), déclarée dans son `Cargo.toml` : fenêtres, premier plan et `SendInput` (4.1), interface de barre (4.5b). |

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
| `unlock_elevation` | Tâche 4.5a : `begin_elevation_disable` (contexte `disable-elevation`, 60 s, un usage) et `complete_elevation_disable` (approbateur, clé enrôlée, contexte vivant, deux signatures du même message ; retire la clé via `device_list::disable_elevation`). Code `elevation-not-enrolled`. |
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
| `agent_unlock` | `request-unlock`, `decide-unlock`, `approval-context`, `hello-*`, `elevation-*`, dont `elevation-disable` et `elevation-begin {purpose:"disable"}` (environnement de `UnlockRequests` : session, aléa, heure, appareils, politique ; appareils et politique modifiés sont enregistrés). |
| `item_value` | Adaptateur unique `Item`/`Folder`/`Organization` vers les `Value` lues par les projections de `vaultagent-core` (les projections de core restent sur `Value`) : `attachments` devient un tableau quand `has_attachments`. |

Tests 4.5a : `routes_elevation` (retrait de bout en bout avec de vraies signatures de la fixture du coeur, refus par cause, contexte à usage unique, `purpose` absent inchangé) ; cas dorés `elevation-disable-begin`, `-refused`, `-bad-proof` (client `elevated` du harnais ; le succès signé n'est pas doré car le harnais ne signe pas) ; coeur : `tests/unlock_elevation_disable.rs`.

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
parcours configure, code, association, statut agent, front ; proxy ; limites ; connexion inactive). `tests/golden.rs` rejoue les 52 cas.

## `vaultagent-server` : image Docker et sonde de santé (tâche 2.5a)

`rust/Dockerfile` : le contexte est la **racine du dépôt** (le Dockerfile copie `rust/` et les trois fichiers de licence).
Étapes : `base` (`rust:1.100.0-bookworm` épinglée par digest, copie de `rust/`), `test` (`cargo test --locked -j 1` sans informations de débogage, puis
`rm -rf target` dans la même couche, sinon la couche du dossier `target` sature le disque ; avec `-j 2` et le débogage, `rustc`
manque de mémoire sur la VM Agents de 3,8 Go, le 17/11/2026 : arrêter aussi les conteneurs de test avant de construire), `builder`
(`cargo build --release --locked -p vaultagent-server`), puis l'image finale `debian:bookworm-slim` (digest) avec
`ca-certificates`, le binaire `/usr/local/bin/vaultagent-server`, `LICENSE`, `LICENSE-GPL.txt` et `THIRD-PARTY.md` dans `/app`.
`USER 1000:1000`, `EXPOSE 8080`, `/data` appartient à 1000. Aucun script ni Node ; `assets/style.css` est dans le binaire.
Le Docker de la VM Agents n'a pas BuildKit : `DOCKER_BUILDKIT=0`, `rust/Dockerfile.dockerignore` n'y est alors pas lu, c'est
l'archive envoyée qui exclut `target`.

```
docker build --target test -f rust/Dockerfile .
docker build -f rust/Dockerfile -t vaultagent-rust:2.5a .
```

`vaultagent-server healthcheck` (module `healthcheck`) : `GET /healthz` sur `127.0.0.1:$VAULTAGENT_PORT` (8080 par défaut)
par `TcpStream` (délai de 3 s), code de sortie 0 si le statut est 200, sinon 1. C'est le `HEALTHCHECK` de l'image.
Tests : modules `#[cfg(test)]` (200, 503, réponse invalide, port fermé, port par défaut) et `tests/healthcheck.rs` (contre `serve`).

## `vaultagent-parity` : outil de parité Node et Rust (tâche 2.5b)

Crate `publish = false`, hors de l'image. Elle joue le même scénario (60 requêtes) sur le serveur Node et le serveur Rust,
par `TcpStream` en clair vers deux ports locaux (tunnels ssh), puis compare statut, en-têtes `content-type`,
`cache-control`, `x-content-type-options` et corps. Scénario : `healthz`, admin (`state`, `configure`, `unlock`, `pairing`),
association par demande (`pair-request`, `poll`, `pair-decide`) et par code, routes agent (`status`, `vaults`, `catalog`,
`write-preview`, `write` créer, rejouer, mettre à jour, supprimer, `seal` ouvert avec la clé de test, `sharing`, `share`
en aperçu, `lock`, `request-unlock`, `decide-unlock` signé RSA-PSS), erreurs (origine, JSON requis, route, méthode,
non autorisé, propriétaire), `revoke` et `lock`. Les clés, signatures et enveloppes viennent de `vaultagent-core`
(`rsa_keys` : `generate_rsa`, `public_key_spki`, `sign_pss`, `sign_decision`).

Normalisation (`normalize.rs`) : jetons, secrets, UUID, dates, codes, empreintes, `requestId` aléatoires et nombres variables
deviennent des marqueurs ; les listes de fiches, dossiers, collections et organisations sont triées (l'ordre n'est pas
garanti par le protocole). Un écart affiche l'étape, le chemin et les deux valeurs normalisées, jamais un secret.
Écarts acceptés (`accepted.rs`) : le faux dossier `No Folder` que Node ajoute à `write-preview` (artefact de la CLI Bitwarden).

```
ssh -N -L 8390:127.0.0.1:8390 -L 8391:127.0.0.1:8391 monsieurz@10.10.60.10
cargo run --release -p vaultagent-parity -- --node http://127.0.0.1:8391 --rust http://127.0.0.1:8390 [--trace] [--pause 65]
```

Code de sortie 0 si aucun écart refusé, 1 sinon, 2 pour une erreur d'usage ou réseau. `--trace` liste étape, statuts et codes
d'erreur ; `--server`, `--email`, `--password` et `--owner` ont pour défaut le Vaultwarden jetable de la VM Agents.
Prérequis sur la VM Agents : Vaultwarden jetable derrière Caddy `tls internal` (`vaultagent-test-tls`, alias
`vaultagent-test-vw.test`), serveurs `vaultagent-rust-test` (8390) et `vaultagent-node-test` (8391) avec `VAULTAGENT_ORIGIN=https://localhost:<port>`,
la racine Caddy montée en `SSL_CERT_FILE` (Rust, rustls) et `NODE_EXTRA_CA_CERTS` (Node ; le moteur Node filtre `NODE_*`, un
`engine.mjs` corrigé d'une ligne est monté en lecture seule). **Vaultwarden limite les connexions** (rafale de 10, puis une par minute,
HTTP 429) : un passage consomme 6 connexions, attendre quelques minutes entre deux passages, et recréer les deux serveurs
(volumes neufs) avant chaque passage (`~/vaultagent-test/recreate.sh <étiquette>` sur la VM).

## `vaultagent-server` : front HTML sans JavaScript (tâche 2.4b)

Aucune dépendance nouvelle ni variable d'environnement (`VAULTAGENT_WEB` est supprimée : la feuille de style est dans le
binaire par `include_str!`, `assets/style.css`). Le front de `linux/web/` n'est plus servi : `/app.js` et `/index.html` rendent `404`.

- **`GET /`** : page complète rendue depuis l'état du coffre `default` (même donnée que `GET /api/admin/state`), pour le
  propriétaire Authentik derrière le proxy de confiance. Tâche 7.3 : une page principale sans onglets (barre d'état avec pastille
  « Verrouillé » / « Déverrouillé jusqu'à HH:MM » / « À configurer », « Déverrouiller » avec champ 2FA ou « Verrouiller », langue,
  « Aide » ; ligne du coffre « <e-mail> sur <hôte> · verrouillage après N min · Modifier » ; section « À valider » seulement si une
  demande de déverrouillage ou d'ajout attend ; « Appareils » avec rôle « validateur · UAC | Hello | carte » ou « agent », « Sans
  demander » pour un agent seulement, date « ajouté le JJ/MM », « Retirer »). Tâche 7.4 : grille sur `100vh` sans défilement
  à 1366 × 768 (`style.css`, 5 Ko, thème sombre par `prefers-color-scheme`) ; appareils paginés par `?page=N` (8 par page, moins quand
  des demandes attendent, minimum 3 ; page invalide ou hors plage bornée, jamais d'erreur ; liens « ‹ Précédent » / « N / M » / « Suivant › »
  seulement s'il y a plusieurs pages ; une redirection après action revient à la page 1) ; « Retirer » est un lien `?confirm=<id>` (GET,
  aucune action) qui ouvre sur la ligne « Il ne pourra plus rien remplir. », « Confirmer le retrait » (POST `revoke`, contrôle d'origine)
  et « Annuler » (un `confirm` inconnu est ignoré) ; rafraîchissement de 10 s seulement sur une page principale sans demande, sans
  message et sans confirmation ouverte. Pages secondaires `?view=vault` (formulaire du coffre, aussi rendu par `/` tant
  que rien n'est configuré), `?view=add` (nom, code d'ajout, téléchargements avec SHA256) et `?view=help`, chacune avec un lien
  « Retour ». Un ancien `?tab=` est ignoré ; un `?view=` inconnu rend la page principale.
  Messages : `?msg=saved|refused|login-refused|duplicate-account|expired`, des codes traduits au rendu.
- **Langue** : `?lang=fr|en|es` pose le cookie `vaultagent-language` (`Path=/; SameSite=Strict; HttpOnly; Secure`) ; français par défaut.
- **Sécurité** : aucun `<script>` ni attribut `on*` ; `Content-Security-Policy: default-src 'none'; style-src 'self'; img-src 'self';
  form-action 'self'; frame-ancestors 'none'; base-uri 'none'`, plus `no-store`, `nosniff`, `same-origin` (avec `no-referrer`, les formulaires partiraient avec `Origin: null`). Toute donnée insérée passe par
  `front_escape::escape`. Mot de passe et codes 2FA : jamais renvoyés (champs vides), jamais dans une URL.
- **Actions** : formulaires `POST /admin/<configure|unlock|lock|pairing|pair-decide|decide-unlock|auto-unlock|revoke>` en
  `application/x-www-form-urlencoded`. Le propriétaire est contrôlé par `route`, l'en-tête `origin` doit valoir `VAULTAGENT_ORIGIN`
  (sinon `403 origin-refused`), puis `admin::act` (la même fonction que la route JSON) est appelée et la réponse est un `303` vers
  `/?msg=<code>` (page principale) ou `/?view=vault|add|help&msg=<code>` (`configure` vers `vault`, `pairing` en échec vers `add`, `unlock`, `lock` et `decide-unlock` vers le champ caché `view` du formulaire s'il existe) ; les erreurs de `front_error` vont vers `/?msg=<code>`. `pairing` répond directement une page `200` `no-store` (code dans un `<details>`) : le code n'entre dans aucune URL.
- **Rafraîchissement** : `<meta http-equiv="refresh" content="10">` sur la page principale seulement, quand aucune demande n'attend et
  qu'aucun message n'est affiché ; jamais sur les pages secondaires ni sur le formulaire du coffre (une saisie serait perdue). Lien « Actualiser » dans la barre.
- **Rôle des appareils** : `device_kind` lit l'enregistrement (clé de validateur, clé d'élévation, justificatif Hello) ; `State.device_kinds` ne
  sort pas dans le JSON de `GET /api/admin/state` (`#[serde(skip)]`).
- **Vocabulaire** : appareil, ajout d'appareil, validateur, Déverrouiller, Sans demander, Retirer (table de `docs/rust/interface-web.md`).
- **Dates** : rendues côté serveur en heure de Paris, sans mention de fuseau (`HH:MM`, `JJ/MM` pour les appareils) ; `front_paris` calcule
  UTC+1 ou UTC+2 (dernier dimanche de mars 01:00 UTC au dernier dimanche d'octobre 01:00 UTC) sans base de fuseaux.

| Module (2.4b) | Rôle |
|---|---|
| `front_page`, `front_view`, `front_screen` | `GET /` : choix de langue, d'écran (`Screen` : principale, `vault`, `add`, `help`) et de message, puis assemblage de la page (rafraîchissement). |
| `front_bar`, `front_vault_line`, `front_pending`, `front_devices`, `device_kind` | Organismes de la page principale : barre d'état (et barre « Retour » des pages secondaires), ligne du coffre et message, « À valider », « Appareils » et rôle des appareils. |
| `front_account`, `front_add`, `front_help` | Pages secondaires : formulaire du coffre, ajout d'un appareil, aide. |
| `front_html`, `front_escape` | `View` (contexte de rendu), champs cachés, balise de formulaire ; `escape` (cinq caractères). |
| `front_action`, `front_body`, `form` | `POST /admin/*` : contrôles, `Form::parse` (analyseur `x-www-form-urlencoded`, std seulement), corps JSON de l'action, redirection. |
| `front_error`, `i18n_errors` | Tâche 7.2 : erreur d'un formulaire `POST /admin/*` du navigateur (type de formulaire ou `Sec-Fetch-Mode: navigate`) = redirection `303` avec un code de message, sans action (`origin-refused`, `request-size`, `request-refused`, `route-refused` compris ; `proxy-required`, `owner-required`, `unauthorized` restent en JSON) ; erreur d'une page `GET` du navigateur (`Sec-Fetch-Mode: navigate` ou `Accept: text/html`) = page HTML d'une phrase avec lien « Retour » et en-têtes de sécurité. Les routes `/api/admin/*` et les requêtes sans ces en-têtes gardent le JSON du protocole. `Notice::from_error` est la table code d'erreur → message (code inconnu : « Action refusée par le serveur. »). |
| `front_pairing`, `front_response`, `front_notice`, `front_lang`, `front_dates`, `front_paris`, `front_paging` | Page du code ; réponses (CSP, redirection 303, `style.css`) ; messages ; langue et cookie ; dates en heure de Paris (`front_paris` : décalage, calendrier) ; pagination des appareils. |
| `i18n`, `i18n_layout`, `i18n_account`, `i18n_devices`, `i18n_help`, `i18n_errors` | Tables fr/en/es indexées par le texte français (80 entrées après la tâche 7.3) ; un texte absent reste en français. |

Tests 2.4b, 7.3 et 7.4 (`front_paging` : pagination, confirmation du retrait, heure de Paris, feuille de style) : `front_pages` (pages en trois langues, ancien `?tab=` ignoré, échappement, champs vides, aucun script, cookie), `front_layout` (barre, ligne du coffre, « À valider », rôles et modes, pages secondaires et « Retour », rafraîchissement, vocabulaire), `front_actions`
(origine, propriétaire, corps, chaque action, page du code), tests unitaires de chaque module, `transport` (`GET /` sur socket réel).
Le cas doré `front-index` décrit la nouvelle page (CSP sans script) ; les 48 autres sont inchangés.

## Binaire Linux du client (tâche 3.7)

Construit sur la VM Agents dans `rust:1.100.0-bookworm` (`cargo build --release -p vaultagent-client`, 4,7 Mo, glibc), installé
dans `~/.local/opt/vaultagent-rust/vaultagent` à côté du client Node (`~/.local/bin/vaultagent-client`, inchangé). Il relit
`~/.config/vaultagent/client.json` sans nouvelle association. Vérifié le 16/11/2026 sur le serveur réel : `status` (0), `vaults`,
`catalog` verrouillé (4, `locked`), origine invalide (1, `usage`). Le remplacement du lien `~/.local/bin` est l'affaire de la phase 5.

## `vaultagent-client` : configuration, association, lectures, écritures, déverrouillage, `seal`, aide (tâches 3.1 à 3.6)

Bibliothèque `vaultagent_client` et binaire `vaultagent`. Dépend de `vaultagent-core` (`HttpsAddress`, `https_origin`, `parse_bearer`, `Uuid`), `serde`, `serde_json`, `zeroize`, `ureq` (rustls, magasin de certificats du système) et `clap`.
Remplace `remote-client/cli.mjs` et `agent-tools.mjs` pour Linux ; le remplissage de champ de navigateur (Playwright) n’existe pas en Rust : `fill_with` en est la brique.

```
vaultagent [--config <fichier>] [--vault <id>] pair-request <https-serveur> [nom]
vaultagent [--config <fichier>] [--vault <id>] status | vaults | sharing | catalog [recherche] | preview [id]
vaultagent create --operation-id <uuid> --metadata <json> [--values-stdin]
vaultagent update <itemId> --operation-id <uuid> --expected-revision <date> [--organization-id <uuid>] [--metadata <json>] [--values-stdin]
vaultagent delete | restore <itemId> --operation-id <uuid> --expected-revision <date> [--organization-id <uuid>]
vaultagent folder create <nom> | rename <folderId> <nom> | delete <folderId>   (chacun avec --operation-id <uuid>)
vaultagent [--vault <id>] unlock [--wait <secondes 0-300>] | lock
vaultagent [--vault <id>] seal <itemId> <username|password|totp> <origine-https> --public-key <spki-base64>
vaultagent foreground | focus --process <exe> --title <fragment>   (Windows)
vaultagent [--vault <id>] type <itemId> <username|password|totp> <origine-https> --process <exe> --title <fragment>   (Windows)
```

- **Configuration** : `~/.config/vaultagent/client.json` = `{"server","token"}`, identique au client Node (une VM déjà associée n'a rien à refaire). `--config` la remplace (fichier en clair). Sous Windows sans `--config`, voir « Stockage DPAPI (tâche 4.2) ».
  Sous Unix, le fichier est refusé si `mode & 0o077` n'est pas nul ou si son propriétaire n'est pas l'utilisateur courant (uid lu sans `unsafe` sur le propriétaire de `/proc/self`, Linux ; ailleurs, le mode seul). Écriture atomique : dossier `0700`, temporaire `0600`, `rename`.
- **Sorties** : stdout, le corps JSON du serveur sur une ligne. stderr, une ligne `{"version":1,"error":"<code>","message":"<texte>"}` (code du protocole repris : `locked`, `vault-required`, `unauthorized`…, sinon `usage`, `config`, `unreachable`, `indeterminate`, `bad-response`, `pairing-denied`, `pairing-expired`).
- **Codes de sortie** : 0 succès, 1 usage ou donnée invalide, 2 refus du serveur, 3 cible refusée (réservé à la saisie), 4 coffre verrouillé (HTTP 423), 5 résultat indéterminé (délai ou coupure après l'envoi).
- **Délais** : connexion 10 s, requête 110 s, association 15 s ; redirections refusées ; réponse de 4 Mio au plus. Adresse : `https` seulement, sans identifiants, chemin `/`, ni requête ni fragment.
- **`pair-request`** : affiche le code de vérification sur stderr, interroge toutes les 2 s jusqu'à l'expiration de la demande, puis enregistre la configuration et écrit `{"paired":true,"vaultId":…}`. Nom par défaut : `/etc/hostname`, sinon `HOSTNAME`, `COMPUTERNAME`.

| Module | Rôle |
|---|---|
| `exit_code`, `error` | `ExitCode` (0 à 5), `ClientError` (code, message sans secret, code de sortie, `error_line`). |
| `server_address`, `token` | `ServerAddress` (origine validée), `Token` (`Zeroizing`, `Debug` masqué, forme `va1.<uuid>.<43>`). |
| `config`, `config_path`, `private_file` | `ClientConfig::load`/`save`, emplacement (`--config`, `HOME`), contrôle de permissions et écriture atomique. |
| `transport`, `https_transport`, `fake_transport` | Trait `Transport` (`post(chemin, bearer, corps, délai)`), `HttpsTransport` sur `ureq`, `FakeTransport` (réponses scriptées, journal des requêtes, sans réseau). |
| `api` | `agent_call`, `pair_call`, `classify` (statut HTTP vers erreur typée), `map_transport`. |
| `pairing`, `pairing_wire`, `pause`, `machine_name` | `pair_request` (attente `Pause` injectée, `RecordedPause` pour les tests), corps serde de `/api/pair`, nom de machine. |
| `write`, `write_args`, `write_body`, `write_metadata`, `write_values` | `WriteCommand` (7 écritures, corps identiques à `agent-tools.mjs`), arguments `clap`, structures `serde` du corps `write`, clés de métadonnées admises (création : `type`, `name`, `folderId`, `login.uris`, `organizationId`, `collectionIds` ; mise à jour : `name`, `folderId`, `addUris`, `removeUris`), `SecretValues` (entrée standard 64 Kio, `Debug` masqué, effacement au `Drop`). |
| `unlock` | `unlock` (attente bornée, `Pause` injectée, durée comptée par les pauses) et `lock`. |
| `reads`, `cli`, `run` | `ReadCommand` (routes `status`, `vaults`, `sharing`, `catalog`, `write-preview`), définition `clap`, `run` (stdout, stderr, code de sortie) avec `Runtime` injecté. |
| `seal`, `fill` | `SealArgs`, `check_target`, `seal_body`, `run_seal` (commande `seal`) ; `fill_with` (brique des remplissages de la phase 4) et `FillReport`. |
| `help` | Textes d'aide anglais : tableau des codes de sortie (aide générale) et un exemple par commande. |

- **`seal <itemId> <field> <origin> --public-key <spki>`** : aucune valeur en clair ne sort du binaire (une valeur sur stdout irait au modèle). Valide sans réseau : `itemId` UUID, `field` parmi `username`, `password`, `totp`, `origin` = origine HTTPS exacte (sans chemin ni barre finale), clé SPKI RSA de 2048 à 4096 bits (essai de scellement d'une valeur vide par `vaultagent_core::seal`, aucune crypto écrite ici). Puis relaie `seal` et écrit l'enveloppe `{encryptedKey, nonce, tag, data}` telle que le serveur l'a rendue ; seul le détenteur de la clé privée peut l'ouvrir.
- **`fill_with(transport, config, vault, item, field, origin, sink)`** : valide, génère une paire RSA 2048, appelle `seal`, ouvre l'enveloppe (`open_envelope`) et passe la valeur au `sink` (`FnOnce(&Zeroizing<String>) -> Result<(), ClientError>`). Rend `{"filled":true,"field":…,"clipboardUsed":false}` ; clé privée (`ZeroizeOnDrop`) et valeur effacées à la sortie. Enveloppe illisible : `bad-response`.
- **Aide** : `--help` de chaque commande en anglais avec un exemple ; l'aide générale porte le tableau des codes de sortie ; `--version`. Aide et version sortent sur stdout (code 0). Sinon stdout ne reçoit que le JSON du serveur ou l'enveloppe, jamais autre chose.
- **Aucune dépendance nouvelle** : `base64` et `rsa` ne sont pas entrées dans la crate (tout passe par `vaultagent-core`).

Codes d'erreur du client (ligne `{"version":1,"error":<code>,…}` sur stderr) :

| Code | Sens | Sortie |
|---|---|---|
| `usage` | Argument, commande ou donnée invalide ; rien n'est envoyé. | 1 |
| `config` | Configuration absente, illisible, non privée ou jeton mal formé. | 1 |
| `locked` | Coffre verrouillé (HTTP 423). | 4 |
| `indeterminate` | Délai ou coupure après l'envoi, ou écriture sans réponse (garder l'`operationId`, relire `preview`). | 5 |
| `write-indeterminate` | Le serveur signale une écriture au résultat inconnu. | 5 |
| `unreachable` | Rien n'a été envoyé : résolution, connexion refusée ou certificat refusé. | 2 |
| `bad-response` | Réponse invalide, redirigée, de plus de 4 Mio, ou enveloppe illisible. | 2 |
| `pairing-denied` | Association refusée par l'administrateur. | 2 |
| `pairing-expired` | Association expirée sans réponse. | 2 |
| autre code du protocole | Refus du serveur repris tel quel (`vault-required`, `unauthorized`, `origin-refused`, `invalid-field`, `invalid-key`…, sinon `request-refused`). | 2 |

Le code 3 (cible refusée) est rendu par `focus` et `type` (tâche 4.1), avec le rapport de refus sur stdout, pas sur stderr.

- **Écritures** : `--operation-id` obligatoire et fourni par l'appelant (jamais généré). Le reçu part sur stdout tel quel. Les métadonnées ne portent aucun secret (clé hors liste : code 1, « secrets belong in --values-stdin ») ; les secrets (`username`, `password`, `totp`, `notes`) passent seulement par `--values-stdin` (objet JSON, 64 Kio au plus ; pour un `type` différent de 1, `notes` seul). Rien n'est envoyé si un argument est invalide. Délai ou coupure après l'envoi, ou `write-indeterminate` du serveur : code 5 et message « garder l'operationId et relire preview avant toute reprise », sans jamais rappeler une valeur.
- **`unlock [--wait <s>]`** : attente de 0 à 300 s (sinon code 1), `request-unlock` puis, si `pending` ou `approving`, `status` toutes les 2 s jusqu'à l'échéance ; jamais de nouvelle demande après refus ou expiration. Sortie de `agent-tools.mjs` (`{requestId, state, unlocked}` et champs repris) ; code 0 déverrouillé, 4 verrouillé (`waiting`, `expired`, demande en attente sans attente), 2 `denied` ou `failed`. **`lock`** : rend `{"locked":true}`.

Tests : `seal_commands` (aller-retour factice avec sentinelle absente de toute sortie, refus de champ, d’origine, d’UUID et de clé, enveloppe relayée telle quelle, code 4), `exit_codes` (codes 0, 1, 2, 4, 5 et forme stdout/stderr, bibliothèque), `binary` (vrai binaire : aide et tableau des codes, exemples, serveur injoignable = code 2, usage), unitaires par module, `write_commands` (un test par commande et par corps, refus des métadonnées et entrées invalides, sentinelle absente de stdout et stderr, code 5), `unlock_commands` (immédiat, approuvé pendant l'attente, refusé, échoué, expiré, échéance, attente hors bornes, `lock`), `golden_writes` (cas dorés `write-create`, `write-update`, `write-delete`, `write-create-folder`, `request-unlock-*`, `lock`), `run_commands` (commandes de bout en bout sur faux serveur, association approuvée, refusée et expirée, jeton absent de toute sortie), `golden_reads` (rejeu des cas dorés `status-*`, `vaults`, `catalog-*`, `sharing-locked`, `write-preview-*`, `agent-unauthorized`, corps de `pair-request`), `binary` (vrai binaire : usage, aide, version). Les tests de permissions Unix (`0600`, `0700`, refus de `0644`) sont sous `#[cfg(unix)]` : ils ne s'exécutent que sous Linux.

## `vaultagent-win32` et commandes de fenêtre (tâche 4.1)

`vaultagent-win32` (`#![cfg(windows)]` : crate vide ailleurs, `unsafe` déclaré dans son `Cargo.toml`, décision `docs/decisions/2026-11-15-unsafe-win32.md`) n'expose que des fonctions sûres ; chaque bloc `unsafe` est dans une petite fonction au docblock `Sûreté :`. Dépendances : `windows-sys` 0.61 (4 modules Win32) et `zeroize`.

| Module | Rôle |
|---|---|
| `error` | `Win32Error` (`NoForegroundWindow`, `UnreadableProcess`, `Api {call, code}`). |
| `native` (privé) | Appels bruts de lecture : `GetForegroundWindow`, pid, titre, chemin d'image (`QueryFullProcessImageNameW`), `EnumWindows`. |
| `window` | `WindowInfo {handle, process_id, process, title}`, `foreground()`, `visible_windows()`, `describe(handle)`, `process_name_from_path` (sans dossier ni `.exe`). |
| `raise` | `bring_to_front(handle)` : restauration si réduite, mouvement de souris nul, entrée attachée au fil du premier plan, `SetForegroundWindow`, attente de 150 ms. L'appelant relit le premier plan : seule preuve qui compte. |
| `input` | `type_text(&Zeroizing<String>, check)` : `SendInput` Unicode (`KEYEVENTF_UNICODE`, appui puis relâchement, paires de substitution comprises) ; `check` est appelé **avant chaque caractère**, un refus arrête la frappe (`TypeError::Refused {reason, typed}`). Les événements sont effacés après envoi. |

Tests : unitaires (nom de processus, refus avant le premier caractère, lecture seule du premier plan et des fenêtres visibles) et quatre tests Win32 réels `#[ignore = "fenetre factice du test"]` (`cargo test -p vaultagent-win32 -- --ignored --test-threads=1`) : ils créent leur fenêtre factice (`STATIC` de premier niveau titrée, contrôle `EDIT` enfant), la mettent au premier plan, tapent une valeur factice et la relisent. Le contrôle de cible de chaque frappe exige que le premier plan soit cette fenêtre ; sinon rien n'est envoyé et le test le rapporte sans taper.

Côté `vaultagent-client` (logique pure compilée partout, commandes réelles sous Windows) :

| Module | Rôle |
|---|---|
| `text_fold` | `fold` : minuscules et lettres accentuées ramenées à leur base (table du français et de l'anglais, marques combinantes retirées), sans dépendance. |
| `target_requirement` | `TargetRequirement::create(process, title)` : nom simple (64 caractères, lettres, chiffres, `.`, `_`, `-`, `.exe` retiré), fragment de 3 caractères au moins sans caractère de contrôle, titres génériques de navigateur refusés (comparaison sans accents ni casse) ; `matches` (processus exact sans casse, titre contenant le fragment sans casse). La lecture de `--process` et `--title` est faite par `clap` (`window_args`). |
| `target_check` | `WindowSnapshot`, `verify` (fenêtre active = fenêtre capturée **et** conforme), `select_unique` (zéro ou plusieurs correspondances refusées), `verify_activated`. |
| `target_report` | `WindowDescription`, `TargetRefusal`, lignes JSON : `{"process","title"}`, `{"typed":true,"field","clipboardUsed":false,"target"}`, `{"focused":true,"target"}`, refus `{"typed":false|"focused":false,"reason","expected","foreground"}` (`foreground` vaut `null` si illisible ; pas de clé `version`, comme Node). |
| `window_system`, `windows_system` | Trait `WindowSystem` (`foreground`, `visible`, `bring_to_front`, `type_text`) ; `system()` rend le système Windows, `Usage` ailleurs ; `WindowsSystem` adapte `vaultagent-win32`. |
| `window_commands`, `window_args` | `foreground`, `focus`, `type_text` ; arguments `clap`. |

- **`foreground`** : décrit la fenêtre active (`{"process","title"}`) sans serveur ni configuration.
- **`focus --process --title`** : active l'unique fenêtre visible qui correspond, puis relit le premier plan. Zéro, plusieurs correspondances ou activation refusée par Windows : code 3, `{"focused":false,…}`.
- **`type <itemId> <field> <origin> --process --title`** : valide les arguments, vérifie le premier plan, puis `fill_with` avec un puits qui revérifie la cible avant chaque caractère, tape et laisse `Zeroizing` effacer la valeur. Refus de cible (avant la demande ou pendant la frappe) : code 3 et `{"typed":false,"reason","expected","foreground"}` sur stdout, forme lue par `checkTyped` de Node. Hors Windows ces trois commandes répondent `usage` (code 1). Elles utilisent la configuration de `--config`, ou sans cette option le stockage DPAPI (4.2).
- Nouveaux codes d'erreur (stderr) : `target-refused` (porté par `ClientError::TargetRefused`, converti en rapport code 3 par `type`) et `window` (frappe refusée par Windows, code 2).

## Stockage DPAPI (tâche 4.2)

`vaultagent-win32::dpapi` : `protect(&[u8], entropie, Scope)` et `unprotect(…) -> Zeroizing<Vec<u8>>` (`CryptProtectData`, `CryptUnprotectData`, toujours avec `CRYPTPROTECT_UI_FORBIDDEN`) ; `Scope::CurrentUser` ou `Scope::LocalMachine` (réservée à 4.3). La mémoire rendue par Windows est copiée dans un `Zeroizing`, écrasée de zéros puis libérée par `LocalFree`.

`vaultagent-core::private_key` : `export_pkcs8` et `import_pkcs8` (clé RSA de 2048 bits exactement, `invalid-key` sinon). La signature RSA-PSS SHA-256 de sel 32 et le message `["VaultAgent.Unlock.v2", …]` existaient déjà (`sign_pss`, `decision_message`) : rien n'est dupliqué.

`vaultagent-client` (logique compilée partout, DPAPI réelle sous Windows seulement) :

| Module | Rôle |
|---|---|
| `protector` | Trait `Protector` (`protect`, `unprotect`), `DpapiProtector` (utilisateur courant, entropie `VaultAgent.Remote.v1`), `system_protector()` (erreur `config` hors Windows). |
| `protected_config` | `load_protected` et `save_protected` : `remote-client.json` = `{"server","protectedToken":"<base64>"}`, jeton UTF-8 protégé ; écriture atomique. Même fichier que le client .NET. |
| `approver_key` | `ApproverKey::load_or_create(chemin, protecteur)` : PKCS#8 DER protégé (`approval-key.dpapi`), créé une fois (lien dur d'un temporaire : jamais d'écrasement), fichier corrompu refusé sans être modifié ; `approval_public_key` (SPKI base64) et `approval_sign(message)` (RSA-PSS SHA-256, sel 32). |
| `config_store` | `ConfigStore::locate(--config, HOME, dossier de données)` : `Plain` (fichier en clair) ou `Protected` (dossier `%LOCALAPPDATA%\VaultAgent`, Windows sans `--config`) ; `load`, `save`, `approver_public_key`. |

- **Sans `--config` sous Windows** : toutes les commandes lisent `remote-client.json` déchiffré en mémoire ; `pair-request` y écrit au même format. `--config <fichier>` garde le format en clair (tests).
- **`pair-request --approver`** : joint `approverPublicKey` (SPKI de la clé d'approbateur, créée si absente) ; refusé (`usage`) avec `--config`.
- `Runtime.local_data` : dossier de données injecté (`main` le calcule depuis `LOCALAPPDATA`, les tests passent un dossier temporaire).

Tests : aller-retour DPAPI réel (non ignoré, aucun fichier de l'utilisateur), lecture d'un fichier écrit comme le .NET, clé d'approbateur (création, relecture, SPKI stable, signature vérifiée par `vaultagent-core`, fichier corrompu refusé), `tests/protected_store.rs` (association, relecture sans `--config`, `--approver`).

Tests du client : `target_requirement`, `text_fold`, `target_check`, `target_report`, `cli` (unitaires) `tests/window_type.rs` et `tests/window_focus.rs` (fausse fenêtre `tests/common/fake_windows.rs` et faux serveur qui scelle une sentinelle : succès sans la valeur en sortie, refus avant toute requête, premier plan changé ou titre dérivé en cours de frappe, arguments invalides sans accès fenêtre ni serveur, `focus` ; lecture seule réelle de `foreground` et de `focus` sur une fenêtre inexistante).

## Clé de validation UAC et décision signée (tâche 4.3)

`vaultagent-win32` : `is_elevated()` (`CheckTokenMembership` sur le SID des Administrateurs, jeton effectif), `current_user_sid()` (SID texte du jeton), `create_private_directory(chemin)` (SDDL `D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)`, sans héritage ; un dossier existant n'est pas modifié) et `run_elevated(exe, args, délai)` (`ShellExecuteExW` verbe `runas`, fenêtre cachée, attente bornée, processus tué au-delà ; `Win32Error::Cancelled` pour `ERROR_CANCELLED`, `Win32Error::TimedOut`).

`vaultagent-client` :

| Module | Rôle |
|---|---|
| `elevation_system`, `windows_elevation`, `fake_elevation` | Trait `ElevationSystem` (`is_elevated`, `current_sid`, `create_private_directory`, `machine_protector`, `run_elevated`), système Windows réel, faux système des tests (aucune invite UAC). |
| `elevation_key` | Clé d'élévation `%ProgramData%\VaultAgent\elevation\<sid>.key` : RSA 2048 PKCS#8, DPAPI `LocalMachine`, entropie `VaultAgent.Remote.v1`, créée sans écrasement ; SID contrôlé contre les chemins piégés. Même fichier que le .NET. |
| `elevation_sign` | Commande cachée `elevation-sign <sid> <message-base64> <fichier>` : refus `elevation-required` (code 2) sans élévation, sinon deux lignes (SPKI, signature RSA-PSS) ; option cachée `--program-data <dossier>` pour les essais réels. |
| `elevated_sign` | `elevated_sign(système, dossier temporaire, message)` : relance `current_exe() elevation-sign …` avec élévation, fichier de sortie aléatoire relu puis supprimé même en cas d'échec ; annulation de l'invite = `elevation-cancelled`. |
| `proofs` | Trait `Proofs` (`elevation`, `hello`), `SystemProofs` (élévation par processus élevé, Hello signé par `HelloSystem` puis vérifié localement), `FakeProofs`. |
| `approval_request` | `ApprovalRequest` et lecture de `status.approvalRequests`. |
| `decide` | `Approver::decide_unlock(demande, approve, remember, preuves)` : signe `decision_message` avec la clé d'approbateur, ajoute `helloProof` et `elevationSignature` si la demande les exige ; un refus est signé sans preuve ni UAC (`remember` forcé à faux). |
| `elevation_enroll` | `Approver::enroll_elevation` : `vaults`, `status`, `elevation-begin`, signature élevée, `elevation-enroll` ; sortie `{"enrolled":<n>}`. |
| `approval_commands` | Commandes `approvals`, `decide`, `elevation-enroll`, `hello-status`, `hello-enroll` et `elevation-sign`. |

Commandes :

- `vaultagent approvals` : `{"approvalRequests":[{vaultId, requestId, challenge, helloRequired, elevationRequired}]}`.
- `vaultagent decide <requestId> --approve [--remember]` ou `--deny` : appelle `decide-unlock` ; la clé d'approbateur vient du stockage protégé (donc `usage` avec `--config`) ; demande inconnue : `request-not-found` (code 2).
- `vaultagent elevation-enroll` : une invite UAC par coffre à enrôler.
- `Runtime` gagne `program_data`, `temp_dir` et `elevation` (injectés ; `main` lit `ProgramData`) ; `hello` en 4.4.

Tests : message, signature et vérification par `vaultagent-core` ; corps `decide-unlock` pour approuver avec UAC, approuver sans exigence et refuser ; DPAPI `LocalMachine` sur une clé de test en dossier temporaire (l'ACL n'est pas testée sans élévation) ; `elevation-sign` refusé sans élévation ; faux transport pour `approvals`, `decide`, `elevation-enroll` avec un enfant élevé simulé. `tests/uac_live.rs` : deux essais `#[ignore]` qui font apparaître une invite UAC (signature réelle dans un dossier de test, annulation) ; à lancer avec Zed : `cargo test -p vaultagent-client --test uac_live -- --ignored --test-threads=1`.

## Windows Hello (tâche 4.4)

`vaultagent-win32::hello` : `hello_available()` (`{available, api_version}`, sans invite), `hello_register(fenêtre, user_id, user_name, rp, origine)` (`Registration` : identifiant et données de l'authentificateur, dont la clé COSE), `hello_sign(fenêtre, id, défi, rp, origine)` (`Assertion` : identifiant, `clientData`, données de l'authentificateur, signature) et `hello_window()` (fenêtre au premier plan). Les liaisons sont celles de `windows-sys` (`Win32_Networking_WindowsWebServices`) ; options : version 1, délai 60 s, plateforme, vérification exigée, attestation aucune ; ES256 et RS256 proposés. Les structures rendues par Windows sont copiées puis libérées par `WebAuthNFreeCredentialAttestation` et `WebAuthNFreeAssertion`. Le `clientData` JSON est composé dans la crate (défi et origine restreints à `[A-Za-z0-9-_.:/]`, sinon `InvalidArgument`) ; le défi de création vient de `BCryptGenRandom`. `unsafe` : `hello_native` seulement.

`vaultagent-client` :

| Module | Rôle |
|---|---|
| `hello_system`, `windows_hello`, `fake_hello` | Trait `HelloSystem` (`status`, `register`, `sign`), système Windows (identifiant d'utilisateur = empreinte SHA-256 du SID en hexadécimal, 64 octets), système sans Hello hors Windows, faux système scripté. |
| `hello_cose` | `credential_from_registration` : contrôle des données de l'authentificateur (hachage du RP, drapeaux UP, UV et données attestées, identifiant) puis lecture CBOR de la clé COSE : EC2 P-256 (`x`, `y`) ou RSA (module, exposant), validée par `hello_public_key` de `vaultagent-core`. |
| `hello_proof` | `prove` : défi `base64url(sha256(message))`, signature par le système, preuve `{credentialId, clientData, authenticatorData, signature}` en base64 standard, puis `verify_hello` de `vaultagent-core` avant tout envoi. |
| `hello_enroll` | `hello_status` et `Approver::enroll_hello` (algorithme `EnrollHelloAsync` : `vaults`, `status`, `hello-begin`, création unique de la clé si le serveur n'en a pas, second `hello-begin`, signature Hello, `hello-enroll`). |

Commandes :

- `vaultagent hello-status` : `{"available":<bool>,"apiVersion":<n>}`, aucune invite, aucun serveur.
- `vaultagent hello-enroll` : une invite pour créer la clé (une fois), puis une invite par coffre ; `{"enrolled":<n>}`. `--vault` limite à un coffre. Erreurs : `hello-unavailable`, `hello-cancelled`, `hello-failed` (code 2).
- `decide --approve` sur une demande `helloRequired` : la preuve Hello est signée puis envoyée dans `helloProof`.

Tests : conversion COSE EC2 et RSA, refus (site, drapeaux, identifiant, courbe, clé hors courbe) ; preuves signées par une clé P-256 de test et vérifiées par `vaultagent-core` (une preuve pour un autre message ou une origine fausse est refusée) ; corps exacts de `hello-enroll` et de `decide-unlock` par le faux transport et un faux système Hello ; `hello-status` réel sur TowerZ (`tests/hello_live.rs`, sans invite). Deux essais `#[ignore]` du même fichier font apparaître l'invite Windows Hello (création puis signature sur le site de test `vaultagent-test.local`, annulation) ; à lancer avec Zed : `cargo test -p vaultagent-client --test hello_live -- --ignored --test-threads=1`.

## Application de barre (tâche 4.5b)

`vaultagent tray` (Windows) remplace l'interface WinForms du client .NET par du Win32 natif, sans .NET ni WebView. Une seule instance par session (mutex `Local\VaultAgentTrayClient`) ; une seconde instance sort sans rien faire.

`vaultagent-win32` (modules `ui_*`, fonctions sûres) : `create_window` et `UiHandler` (procédure de fenêtre qui rend des `UiEvent` sûrs ; gestionnaire gardé par `RefCell`, donc jamais ré-entré), `run_message_loop`, `Poster` (PostMessage depuis un autre fil), minuteries, `WindowRef::add_control` (texte, boutons, case, saisie, police Segoe UI), `TrayIcon` (`Shell_NotifyIconW` : ajout, info-bulle, bulle, retrait au `Drop`), `show_menu` (`TrackPopupMenu`, entrées désactivables), `Icon::shield` (bouclier calculé en mémoire par `shield_pixels` puis `CreateIconIndirect`), `SingleInstance`, `work_area`, `open_url` (`ShellExecuteW`), `registry_text` (HKCU), `user_locale_name`. Tests Win32 réels `#[ignore]` : `ui_live_tests` et un test par fenêtre du client.

| Module du client | Rôle |
|---|---|
| `translate`, `translations.json` | Traduction : le fichier du .NET (clé française, `[anglais, espagnol]`), copié dans la crate et inclus par `include_str!`, plus neuf textes ajoutés (validation, bulle de demande, association). Langue : registre `HKCU\Software\MonsieurZed\VaultAgent\Preferences\Language`, sinon Windows, sinon anglais. |
| `iso_time` | `expiresUtc` ISO 8601 vers secondes Unix, sans calendrier externe. |
| `tray_status` | `TrayRequest` (client, coffre, échéance), `VaultSnapshot`, `fetch_snapshots` (`vaults` puis `status`). |
| `tray_select` | Logique pure : `mode_for(elevationEnabled)` (carte ou UAC), `pick_new` (demandes déjà notifiées, échues, carte ou décision en cours), `latest_open`, `status_text`, `tip_of`. |
| `tray_menu` | Lignes du menu et entrées grisées selon l'état. |
| `tray_actions` | `decide_one_step` (mode UAC : approbation avec invite directe, refus signé si UAC ou Hello est annulé), `reset_all_automatic` (`approval-context` puis refus signé), `lock_all`. |
| `elevation_disable` | `disable_elevation` (`elevation-begin {purpose:"disable"}`, signature élevée, `elevation-disable`) et `switch_validation`. |
| `pairing_flow` | Association complète partagée avec `pair-request`. |
| `tray_jobs`, `tray_worker` | Travaux et réponses ; fil réseau unique (l'association a son propre fil), réponses par boîte partagée et `PostMessage`. Jamais de réseau ni d'invite dans la boucle de messages. |
| `tray_app`, `tray_run`, `approval_card`, `pair_window`, `tray_clock` | Fenêtre cachée de l'icône, point d'entrée, carte, fenêtre d'association, horloge. |

Règle de validation en une étape (décision du 15/11, plan Rust) : le mode vient du serveur (`approvalSecurity.elevationEnabled` du coffre de la demande), sans réglage local. Mode UAC : pas de carte, bulle « client demande l'accès à coffre », invite UAC directe, annuler envoie un refus signé. Mode carte : carte en haut à droite de la zone de travail (au-dessus des autres fenêtres, sans focus, sans bouton de barre des tâches, « Non » bouton par défaut, fermeture à l'échéance) avec Oui, Non et « Toujours accepter les demandes de ce client » ; « Oui avec Windows Hello » si `helloRequired`. Entrée de menu « Validation » : carte vers UAC par `elevation-enroll` (invite UAC), UAC vers carte par le contexte `disable` (invite UAC) puis `elevation-disable` ; une annulation ne change rien.

Menu : état, Demande de déverrouillage, Ouvrir le serveur, Associer ce client (aussi : double clic), Validation, Activer Windows Hello, Verrouiller maintenant, Désactiver l'acceptation automatique, Quitter. Interrogation toutes les 5 s ; une demande n'ouvre qu'une fois sa carte ou son invite.

Tests : unitaires de toute la logique pure (faux transport, fausses preuves) ; Win32 réels uniquement `#[ignore]` (`cargo test -- --ignored --test-threads=1`, à ne pas lancer sans Zed : ils ouvrent une vraie icône, de vraies fenêtres et une carte). `vaultagent tray` ne se lance qu'à la main avec Zed, client .NET arrêté.
