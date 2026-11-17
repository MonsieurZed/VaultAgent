# Protocole HTTP de VaultAgent, version 1 (figé le 15/11/2026)

Tâche 0.3 du [plan de réécriture](plan.md). Ce document décrit ce que fait **aujourd'hui** le serveur Node
(`linux/http.mjs`, 3.0.0-preview.7) ; le serveur et les clients Rust le reproduisent à l'identique. Les cas dorés
de `rust/contrat/*.json` en sont les exemples exécutables (format en fin de document). Un écart entre ce document
et le serveur Node est un bug de l'un des deux : il se tranche par un cas doré avant tout code Rust.

## Transport

- HTTP/1.1 en clair sur le port 8080, derrière le proxy TLS privé. Le client parle HTTPS au proxy.
- Seul `GET /healthz` est accepté d'une autre adresse que `VAULTAGENT_TRUSTED_PROXY` (préfixe `::ffff:` retiré).
- Délais : en-têtes 10 s, requête complète 120 s ; 40 en-têtes au plus.
- Toute réponse JSON porte `content-type: application/json`, `cache-control: no-store`,
  `x-content-type-options: nosniff`. Le corps est un JSON compact (`JSON.stringify`).
- Un corps de requête est exigé pour toute route `POST` : `content-type` commençant par `application/json`
  (insensible à la casse, suivi de `;` ou de la fin), 24 576 octets au plus, JSON valide.

## Ordre des contrôles

1. `GET /healthz` → `200 {"alive":true}`, sans autre contrôle.
2. Adresse distante ≠ proxy de confiance → `403 proxy-required`.
3. En-tête `origin` présent et ≠ `VAULTAGENT_ORIGIN` → `403 origin-refused`.
4. `POST /api/pair` : corps JSON, voir Association (aucune authentification).
5. Chemin commençant par `/api/agent/` : jeton (`401` n'existe pas : `403 unauthorized`), puis méthode `POST`
   (`400 method-refused`), puis corps JSON, puis route.
6. Tout le reste exige `x-authentik-username` égal au propriétaire (`403 owner-required`) : téléchargements,
   fichiers du front, `GET /api/admin/state`, puis routes admin `POST` avec `origin` égal à l'origine du service
   (`403 origin-refused` sinon, y compris pour une méthode autre que `POST`).

## Erreurs

Toute erreur rend `{"error": code}`. Le code est le message de l'erreur s'il suit `^[a-z-]+(?::[a-z-]+)?$`,
sinon `request-refused` (JSON invalide, exception interne). Statut :

| Statut | Codes |
|---|---|
| 403 | `unauthorized`, `owner-required`, `proxy-required`, `origin-refused` |
| 423 | `locked` |
| 400 | tous les autres |

Codes utiles aux clients : `vault-required` (jeton valable pour plusieurs coffres, `vaultId` absent),
`vault-missing`, `json-required`, `request-size`, `route-refused`, `method-refused`, `not-configured`,
`invalid-field`, `origin-refused` (aussi pour une origine absente de la fiche, en 403), `field-missing`,
`totp-period`, `invalid-write`, `invalid-patch`, `invalid-item`, `item-refused`, `revision-conflict`,
`duplicate-item`, `duplicate-folder`, `folder-missing`, `operation-conflict`, `receipt-capacity`,
`write-indeterminate`, `invalid-share`, `organization-refused`, `duplicate-collection`, `share-indeterminate`,
`request-expired`, `approver-required`, `approval-refused`, `elevation-required`, `hello-required`,
`hello-invalid`, `hello-enrollment-refused`, `hello-enrollment-authorization-required`,
`elevation-enrollment-refused`, `elevation-not-enrolled`, `pairing-refused`, `pairing-rate-limit`, `pairing-limit`, `invalid-device`,
`device-limit`, `invalid-key`, `invalid-vault-registry`, `invalid-vault`, `invalid-vault-name`, `vault-limit`, `duplicate-account`, `engine-*` et `<module>:<raison>` (erreurs du moteur relayées).

Règle client (Node et .NET) : `423` → coffre verrouillé ; `400` avec `vault-required` → choisir un coffre ; tout
autre statut non 2xx → refus. Aucune redirection n'est suivie ; une réponse de plus de 4 Mio est refusée.

## Authentification d'un client

`authorization: Bearer va1.<deviceId>.<secret>` : `deviceId` UUID minuscule, `secret` 43 caractères
`base64url` (32 octets). Le serveur garde `sha256(jeton)` en hexadécimal et compare à temps constant. Un jeton
peut valoir pour plusieurs coffres (droit accordé par l'administrateur). `vaultId` dans le corps choisit le coffre ;
absent, il faut un seul coffre autorisé (`vault-required` sinon). Un coffre non autorisé → `403 unauthorized`.

## Routes agent (`POST /api/agent/<route>`)

Corps commun : `{"vaultId"?: string}` plus les champs de la route.

| Route | Corps | Réponse 200 |
|---|---|---|
| `vaults` | `{}` | `[{id, name, version, configured, unlocked, timeoutMinutes, expiresUtc}]` |
| `status` | `{}` | voir ci-dessous |
| `request-unlock` | `{}` | `{state:"unlocked", unlocked:true}` ou `{vaultId, requestId, state, expiresUtc}` (+ `unlocked:true` si acceptation automatique) |
| `decide-unlock` | `{requestId, approve, remember, signature, helloProof?, elevationSignature?}` | `{vaultId, requestId, state, expiresUtc}` ou `{automaticAcceptance:false}` (`requestId:"reset-auto"`) |
| `approval-context` | `{}` | `{vaultId, requestId:"reset-auto", challenge}` |
| `hello-begin` | `{}` | `{vaultId, requestId:"enroll-hello", challenge, helloCredential}` |
| `hello-enroll` | `{credential, helloProof, signature}` | `{helloEnabled:true, helloRequired:true, vaultId}` |
| `elevation-begin` | `{purpose?:"enroll"\|"disable"}` | `{vaultId, requestId:"enroll-elevation"\|"disable-elevation", challenge}` |
| `elevation-enroll` | `{publicKey, signature, elevationSignature}` | `{elevationEnabled:true, vaultId}` |
| `elevation-disable` | `{signature, elevationSignature}` | `{elevationEnabled:false, vaultId}` |
| `catalog` | `{search?}` (200 caractères gardés) | `[{id, name, organizationId, collectionIds, origins, username, password, totp}]` (booléens de présence) |
| `seal` | `{itemId, field, origin, publicKey}` | enveloppe `{encryptedKey, nonce, tag, data}` |
| `write-preview` | `{itemId?}` | sans `itemId` : `{folders:[{id,name}], items, trash}` ; avec : détail |
| `write` | voir Écritures | reçu |
| `sharing` | `{}` | `{organizations:[…]}` (forme du moteur) |
| `share` | voir Partage | aperçu ou reçu |
| `lock` | `{}` | `{locked:true}` |

Autre route sous `/api/agent/` → `400 route-refused`.

**`status`** : `{vaultId, vaultName, version:"3.0.0-preview.7", configured, unlocked, timeoutMinutes,
expiresUtc, approvalSecurity, unlockRequest, approvalRequests}`.
`approvalSecurity` = `{registered, helloEnabled, elevationEnabled, helloEnrollmentAllowed, helloRequiredForAll,
helloRequired}` ; `unlockRequest` = demande du client (`{vaultId, requestId, state, expiresUtc}`) ou `null` ;
`approvalRequests` = `[]` si le client n'est pas approbateur, sinon les demandes `pending` du coffre :
`{vaultId, requestId, state, expiresUtc, challenge, deviceName, vaultName, helloRequired, elevationRequired,
helloCredential}`. États d'une demande : `pending`, `approving`, `approved`, `denied`, `failed`.

**`seal`** : `itemId` UUID, `field` ∈ `username|password|totp`, `origin` = origine HTTPS exacte (sinon
`invalid-field`) ; l'origine doit figurer parmi celles des URI de la fiche (`origin-refused`). `publicKey` = SPKI
DER base64 d'une clé RSA de 2048 à 4096 bits (`invalid-key`). Enveloppe : clé AES-256 aléatoire chiffrée en
RSA-OAEP SHA-256 (`encryptedKey`), nonce 12 octets, étiquette 16 octets, données AES-256-GCM, tout en base64.
Pour `totp`, le serveur attend la période suivante s'il reste moins de 5 s.

**Détail de `write-preview`** : `{id, name, type, folderId, revisionDate, organizationId, collectionIds, deleted,
uris, has:{username, password, totp, notes}}`. Les listes `items`/`trash` portent `{id, name, type, folderId,
revisionDate, organizationId, collectionIds}` (types 1 et 2, fiches archivées exclues).

### Écritures (`write`)

Clés admises : `operationId` (UUID, obligatoire), `action`, `itemId`, `folderId`, `expectedRevisionDate`, `item`,
`patch`, `organizationId` ; toute autre clé → `invalid-write`.

| `action` | Champs |
|---|---|
| `create` | `item:{type:1\|2, name, folderId?, notes?, login?:{username?, password?, totp?, uris?:[{uri}]}, organizationId?, collectionIds?}` |
| `update` | `itemId`, `expectedRevisionDate`, `patch:{name?, folderId?, notes?, username?, password?, totp?, addUris?, removeUris?}`, `organizationId?` |
| `create-folder` | `item:{name}` |
| `delete`, `restore` | `itemId`, `expectedRevisionDate`, `organizationId?` |
| `update-folder` | `folderId`, `item:{name}` |
| `delete-folder` | `folderId` |

Bornes : nom de fiche 500, de dossier 250, sans caractère de contrôle ; notes 12 000 ; champs de connexion 8 192 ;
20 URI au plus, 2 048 caractères, `https:` sans identifiants, requête ni fragment (`addUris` ; `removeUris`
accepte toute URI présente) ; 1 à 32 collections, exigées avec `organizationId`.

Reçu : `{id, operationId, action, verified:true}` plus `revisionDate` pour `create`, `update`, `delete`,
`restore`. Idempotence : clé `<profil>:<operationId>`, empreinte HMAC-SHA-256 (clé machine) de la requête sous
forme canonique (clés triées, `null` retirés, `vaultId` exclu). Même opération, même empreinte → reçu enregistré
avec `replayed:true` ; empreinte différente → `operation-conflict` ; intention sans reçu → `write-indeterminate`.
Après l'intention, tout échec rend `write-indeterminate` : le client garde `operationId` et relit avant de
réessayer (code de sortie 5 de la CLI).

### Partage (`share`)

Clés : `action` (`preview|apply|create-collection`), `organizationId` (UUID), `itemId`, `collectionIds` (1 à 32),
`operationId` (sauf `preview`), `expectedRevisionDate`, `confirmOwnershipTransfer`, `name` (collection, 200).
Aperçu : `{itemId, name, expectedRevisionDate, organizationId, organizationName, collectionIds,
transfersOwnership:true, requiresConfirmation:true}`. Reçu : `{id, operationId, action, organizationId,
verified:true}` ; même idempotence que les écritures (`share-receipts.json`, `share-indeterminate`).

## Association (`POST /api/pair`)

| Corps | Réponse 200 |
|---|---|
| `{action:"request", name, approverPublicKey?}` | `{requestId, secret, verification:"ABCD-EFGH", expiresInSeconds:300}` |
| `{action:"poll", requestId, secret}` | `{state:"pending", expiresInSeconds}`, `{state:"denied"}` ou `{state:"approved", token, deviceId, vaultId}` (une seule fois) |
| `{code:"<vaultId>.<code>"}` ou `{code}` | `{token, deviceId, vaultId}` |

Nom : 80 caractères au plus, caractères de contrôle retirés. 10 ouvertures par minute (`pairing-rate-limit`),
8 demandes en attente (`pairing-limit`), durée 300 s. Codes émis par l'administrateur : 18 octets `base64url`,
5 minutes, 10 essais par minute.

## Routes administrateur

Exigent `x-authentik-username` = propriétaire. `GET /api/admin/state?vaultId=` → `{vaultId, vaultName, version,
configured, unlocked, timeoutMinutes, expiresUtc, account:{server, email, timeoutMinutes}|null, devices:[{id,
name, created, autoUnlock}], unlockRequests, pairRequests:[{requestId, name, verification, approver,
expiresInSeconds}]}`. Les autres sont `POST` avec `origin` = origine du service, corps `{vaultId?, …}` :

| Route | Corps | Réponse |
|---|---|---|
| `configure` | `{server, email, password?, code?, timeoutMinutes:1..1440\|null}` | statut du coffre |
| `unlock` | `{code?}` | statut |
| `lock` | `{}` | `{locked:true}` |
| `decide-unlock` | `{requestId, approve, code?}` | demande publique |
| `auto-unlock` | `{deviceId, enabled}` | `{saved:true}` |
| `pairing` | `{name}` | `{code:"<vaultId>.<code>", expiresInSeconds:300}` |
| `pair-decide` | `{requestId, approve}` | `{saved:true}` |
| `revoke` | `{deviceId}` | `{revoked:true}` (verrouille le coffre) |
| `write-preview`, `write` | comme côté agent | idem |

Fichiers : `GET /`, `/app.js`, `/style.css` (en-têtes CSP stricts, `referrer-policy: no-referrer`) ;
`GET /downloads/{windows-client.zip, linux-client.tar.gz}` et leurs `.sha256` en `application/octet-stream`
(`404 download-unavailable` si absent).

## Retrait de la clé d'élévation (ajout du 16/11/2026, tâche 4.5a)

La version 1 du protocole est étendue de façon rétrocompatible : `purpose` est facultatif dans `elevation-begin`
(absent ou `enroll` : comportement inchangé ; autre valeur que `enroll` ou `disable` : `request-refused`) et la route
`elevation-disable` est nouvelle, avec le code `elevation-not-enrolled`. Les clients et serveurs antérieurs ne
l'appellent pas et ne sont pas touchés.

`elevation-begin {purpose:"disable"}` ouvre un contexte `disable-elevation` de 60 s, à usage unique ; il exige un
approbateur (`approver-required`) dont la clé d'élévation est enrôlée (`elevation-not-enrolled`).
`elevation-disable` vérifie dans l'ordre : approbateur (`approver-required`), clé d'élévation enrôlée
(`elevation-not-enrolled`), contexte vivant (`request-expired`), `signature` de l'approbateur puis
`elevationSignature` de la clé d'élévation enrôlée sur le message `approve:true, remember:false` du contexte
(`approval-refused`). En cas de succès la clé d'élévation est effacée de `devices.json`, `approvalSecurity.elevationEnabled`
passe à faux et les demandes en attente n'exigent plus `elevationSignature`. Format de `/data` inchangé.

## Messages signés

Décision et enrôlements : JSON compact du tableau
`["VaultAgent.Unlock.v2", origin, vaultId, requestId, challenge, approve, remember]`, signé en RSA-PSS SHA-256,
sel 32 octets, par la clé d'approbateur (`signature`) et, si enrôlée, par la clé d'élévation
(`elevationSignature`). `requestId` vaut `enroll-hello`, `enroll-elevation`, `disable-elevation` ou `reset-auto` pour ces contextes.
Preuve Hello : WebAuthn `get`, RP `vaultagent-client.local`, origine `https://vaultagent-client.local`, défi =
`base64url(sha256(message))`, drapeaux UP et UV, signature ECDSA P-256 (DER) ou RSA PKCS#1 v1.5.

## Cas dorés (`rust/contrat/*.json`)

Un fichier par cas, nommé `<route>-<situation>.json` :

```json
{
  "version": 1,
  "name": "status-locked",
  "description": "Un client associé lit l'état d'un coffre configuré et verrouillé.",
  "fixture": { "vault": "locked", "client": "paired" },
  "request": { "method": "POST", "path": "/api/agent/status",
    "headers": { "authorization": "{{token}}", "content-type": "application/json" }, "body": {} },
  "response": { "status": 200, "headers": { "content-type": "application/json", "cache-control": "no-store" },
    "body": { "vaultId": "default", "unlocked": false, "expiresUtc": null, "approvalRequests": [] } }
}
```

- `fixture.vault` : `unconfigured`, `locked` ou `unlocked` (faux moteur avec une fiche factice) ;
  `fixture.client` : `none`, `paired` ou `approver` ; `fixture.proxy` : `trusted` (défaut) ou `untrusted`
  (le proxy de confiance déclaré n'est pas `127.0.0.1`) ; `fixture.downloads` : `false` (défaut) ou `true`.
- Un cas à plusieurs requêtes remplace `request`/`response` par `steps: [{request, response}, …]`, rejouées dans
  l'ordre sur le même serveur (rejeu d'une écriture, conflit d'opération, association puis lecture).
- `request.rawBody` (chaîne) remplace `body` pour un corps non JSON ; `request.bodyPadding` (nombre) ajoute au
  corps une clé `padding` de cette longueur (corps trop gros). Sans `body` ni `rawBody`, aucun corps n'est envoyé.
- Gabarits remplacés avant l'envoi : `{{token}}` (`Bearer` + jeton du client de l'essai), `{{origin}}` (origine du
  service), `{{owner}}` (propriétaire), `{{itemId}}` (fiche factice), `{{rsaPublicKey}}` (clé RSA 2048 publique
  SPKI DER base64 générée par l'essai), `{{operationId}}` (UUID fixe propre au cas).
  Dans un cas à plusieurs étapes, `{{stepN.<clé>}}` (N à partir de 1) est remplacé par la valeur de la clé de premier niveau
  du corps JSON de la réponse de l'étape N (chaîne obligatoire) : `{{step1.requestId}}` alimente la requête 2.
- Le corps attendu est comparé en **inclusion** : chaque clé citée doit exister avec cette valeur ; les clés non
  citées sont libres. Valeurs génériques : `"{{uuid}}"`, `"{{iso-date}}"`, `"{{base64}}"`, `"{{string}}"`,
  `"{{number}}"`. Un tableau attendu est comparé élément par élément, à longueur égale.
  Une valeur générique peut aussi être insérée dans une chaîne plus longue (`"default.{{string}}"`) : la chaîne
  attendue devient une expression régulière ancrée où le générique remplace un motif (`{{number}}` seul exige un nombre JSON).
- Seules des valeurs factices évidentes ; aucun secret réel, aucun réseau hors `127.0.0.1`.
