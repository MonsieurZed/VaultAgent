# Moteur Bitwarden en Rust natif

Date : 2026-11-16. Décision de Zed : « que du Rust ». Le moteur `fork/agent.ts` (fork du CLI Bitwarden,
processus Node enfant) est remplacé par un moteur Rust natif ; le serveur ne lance plus Node.

## Choix

- **Crates officielles du SDK Bitwarden** (`bitwarden/sdk-internal`) plutôt qu'un client maison : la crypto
  (KDF, `EncString`, clés d'organisation) et les formats sont ceux des clients officiels.
- **Licence : branche GPL-3.0** de la double licence `GPL-3.0-only OR LicenseRef-Bitwarden-SDK`. La licence SDK
  limite l'usage aux serveurs distribués par Bitwarden, ce qui exclut Vaultwarden ; la GPL n'a pas cette limite.
  `vaultagent-engine` déclare `license = "GPL-3.0-only"`. Tout binaire distribué un jour le sera sous GPL-3.0 avec
  ses sources ; le fork actuel l'était déjà.
- **Dépendance git épinglée** sur un seul commit pour toutes les crates : `bitwarden-vault`, `bitwarden-sync` et
  `bitwarden-collections` ne sont pas publiées sur crates.io (seule une `bitwarden-vault` 1.0.0 de 2024 l'est).
  Le README amont déclare l'interface du gestionnaire de mots de passe instable : chaque montée de commit
  rejoue la suite d'intégration avant d'être acceptée.
- **Async confiné** : les clients du SDK sont `async` (tokio, reqwest 0.13). `vaultagent-engine` possède son
  runtime tokio et expose une façade **bloquante** ; le serveur reste synchrone (décision
  `2026-11-15-serveur-http.md`), sans tokio dans `vaultagent-server`.
- **Interface typée** : le trait `Engine` remplace l'op générique `call` (routes de `bw serve`) par une méthode
  par opération : connexion (2FA), verrouillage, sync, liste, lecture, TOTP, dossiers, création, mise à jour
  avec contrôle de `revisionDate`, corbeille, restauration, organisations et collections, partage, passkeys.
- **Front web** : HTML rendu par le serveur Rust, formulaires classiques, sans JS. Vérifié le 16/11 : le front
  actuel (`linux/web/app.js`) n'appelle pas WebAuthn ; Hello passe par le client Windows.

## À écrire en plus du SDK (estimation 1 500 à 2 500 lignes hors tests)

Prélogin et jeton d'identité avec 2FA, initialisation de la crypto au déverrouillage, dépôts en mémoire et
`SyncHandler` manquants, recherche, création de collection d'organisation, contrôle de révision, passkeys en
natif (les accesseurs `fido2` du SDK sont derrière la feature `wasm`).

## Résultat du prototype 2.1a (16/11, Vaultwarden 1.37.5 jetable)

GO conditionnel, commit `6238472451e4fa7b5c592b4b24a84e950a258957` (toolchain amont 1.99.0) :

- OK en natif : connexion (`Client.auth().login_password`, prélogin et jeton), crypto initialisée par la
  connexion, dossier et fiche (`vault().folders().create`, `vault().ciphers().create`), sync, liste, lecture,
  TOTP, mise à jour (`ciphers().edit`), corbeille et restauration.
- Colle à écrire (~150 lignes) : poser le `user_id` après connexion (`init_user_id` depuis le `sub` du jeton),
  sinon `NotAuthenticatedError` ; `SyncHandler` des fiches, absent du SDK.
- Inscription : l'aide du SDK (Argon2 et clés V2) crée sur Vaultwarden un compte sans clé privée,
  inutilisable. Le moteur n'inscrit pas de compte ; seuls les tests passent par le chemin PBKDF2 et clé RSA V1.
- Coût : environ 486 paquets, 3 min de compilation à froid, 2,1 Go de cible en debug ; `bitwarden-pm` tire
  tout l'espace de travail amont. Le SDK reste derrière une couche fine dans `vaultagent-engine`.
- `revisionDate` : Vaultwarden refuse une édition sur une copie périmée (HTTP 400, « client copy … out of
  date ») ; `CipherEditRequest.revision_date` est obligatoire. Le moteur resynchronise avant toute écriture.
- Organisations : `share_cipher` partage et la fiche se relit déchiffrée par la clé d'organisation. Le SDK ne
  crée ni organisation ni collection : POST maison (`/api/organizations`, `/api/organizations/{id}/collections`),
  nom de collection chiffré par `vault().collections().encrypt`.
- Sync : le SDK n'enregistre de handler que pour dossiers, Send, politiques et crypto. Le moteur écrit le sien
  pour fiches, organisations (`crypto().initialize_org_crypto`, sinon `MissingKeyId`) et collections (sans
  `Repository<Collection>` amont). `SyncClientExt::sync()` crée un nouveau `SyncClient` à chaque appel : les
  handlers s'enregistrent sur une instance unique gardée par le moteur, jamais sur celle de `bitwarden-pm`.
- Reste à vérifier en 2.1c : 2FA à la connexion. Prototype : `D:\claude-tmp\vaultagent-sdk-spike`.

## Risque principal

Aucune source sur l'usage de `sdk-internal` contre Vaultwarden. La tâche 2.1a (prototype) le vérifie sur un
compte factice avant d'engager le reste ; si elle échoue, on revient vers un client maison (RustCrypto, sans
async) par une nouvelle décision.
