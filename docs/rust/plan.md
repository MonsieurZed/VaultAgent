# VaultAgent en Rust — conception et tâches

Décision de Zed du 2026-11-15 : réécrire en Rust le **serveur Linux**, les **clients Node** et le **client
Windows**, avec une **interface en ligne de commande** pour les agents. Le moteur Bitwarden
(`fork/agent.ts`, SDK TypeScript officiel) **reste tel quel** : il devient un processus enfant du serveur Rust.

## Principes

- **Pas de big bang.** Le serveur et les clients Rust parlent le **même protocole HTTP** que ceux d'aujourd'hui.
  Chaque brique Rust se teste contre l'autre implémentation, puis remplace l'ancienne.
- **Les tests Node sont le cahier de cas.** Chaque test de `linux/test/*.test.mjs` a son équivalent Rust, écrit
  à partir de son intention, pas traduit ligne à ligne. Le protocole figé en tâche 0.3 sert de contrat commun.
- **Aucun secret en sortie** : la CLI renvoie des métadonnées et des reçus ; les valeurs entrent par l'entrée
  standard (jamais en argument) et sortent seulement vers le champ cible.
- Règles de Zed : skill `conduite-code`, `docs/rust-pratiques.md` (Reseau), code en anglais, docblocks en
  français, fichiers de moins de 300 lignes, `Cargo.lock` commité, dépendances justifiées.

## Architecture visée

Espace de travail cargo `rust/` dans ce dépôt (`rust-toolchain.toml`, édition 2024).

| Crate | Rôle | Plateformes |
|---|---|---|
| `vaultagent-core` | Logique pure : formes et patchs d'écriture, registre d'idempotence (trait de stockage), cycle de vie des fiches, demandes d'association, demandes de déverrouillage, signatures RSA-PSS, empreintes. Ni fichier, ni réseau, ni horloge. | toutes |
| `vaultagent-engine` | Processus enfant Node : protocole JSON ligne à ligne de `fork/agent.ts` (op `call`, `safe-update`, `sharing-*`…). | toutes |
| `vaultagent-server` | Bibliothèque + binaire : magasin chiffré, session, routes `/api/agent/*`, `/api/admin/*`, `/api/pair`, fichiers du front. | Linux |
| `vaultagent-client` | Bibliothèque + binaire `vaultagent` : association, lecture, écriture, déverrouillage, remplissage. Sorties JSON versionnées, codes de sortie stables. | toutes |
| `vaultagent-win32` | **Seule crate où `unsafe` est autorisé** : identité et premier plan des fenêtres, `SendInput`, DPAPI, clés NCrypt, WebAuthn Hello, icône de barre et carte d'approbation. Chaque bloc `unsafe` est justifié. | Windows |

Le front web du serveur (`linux/web/`, JS exécuté dans le navigateur) est corrigé en place, pas étendu ni réécrit.

### Interface en ligne de commande (`vaultagent`)

Remplace `vault.mjs`, `agent-tools.mjs` et le client WinForms en mode console. Sous-commandes, toutes avec
`--vault <id>` et `--json` (par défaut pour un non-terminal) :

`pair-request <https-serveur>` · `status` · `vaults` · `catalog [recherche]` · `preview [id]` ·
`unlock [--wait s]` · `lock` · `create` · `update` · `delete` · `restore` · `folder create|rename|delete` ·
`seal` · `type <id> <champ> <origine> --process <exe> --title <fragment>` · `focus --process --title` ·
`foreground` · `tray` (Windows) · `settings` (Windows).

Codes de sortie : 0 succès ; 1 usage ou donnée invalide ; 2 refus du serveur ; 3 cible refusée (fenêtre) ;
4 coffre verrouillé ; 5 résultat indéterminé (reçu à conserver). Les valeurs secrètes de `create` et `update`
se fournissent par `--values-stdin` (un objet JSON `{username,password,totp,notes}`), jamais en argument.

### Décisions prises

- **Unsafe Win32** : crate isolée `vaultagent-win32`. Une note `docs/decisions/` l'explique ; `conduite-check`
  doit apprendre à accepter `unsafe` dans une crate déclarée (tâche 0.2). Toutes les autres crates gardent
  `#![forbid(unsafe_code)]`.
- **Interface Windows** : un binaire Rust unique ; icône de barre et carte d'approbation minimales en Win32.
  Plus de .NET sur TowerZ à la fin.
- **Validation d'une demande, une seule étape** (Zed, 15/11/2026) : réglage du bouclier « Validation : carte »
  ou « Validation : UAC ». Mode carte : Oui/Non seulement, sans invite UAC. Mode UAC : l'invite UAC directement,
  sans carte (bulle d'information avec le nom du client demandeur ; Annuler vaut refus). Passer de UAC à carte
  exige une validation UAC, pour qu'un agent ne puisse pas abaisser la sécurité seul ; le serveur retire alors
  la clé d'élévation enrôlée sur preuve signée par cette clé. Le client .NET actuel enchaîne carte puis UAC.

## Règles d'exécution (comme le plan Trackerr)

- Une tâche = un agent Sonnet neuf, un à la fois ; Opus prépare le brief et vérifie. Les tâches ∥ n'ont pas de
  fichier commun et peuvent tourner ensemble.
- Terminé = le test de la tâche **et** toute la suite `cargo test` passent, `cargo clippy` sans avertissement,
  `conduite-check --crate rust/<crate> --outils` vert. Le rapport cite la dernière ligne de chacun.
- Aucun réseau dans les tests : faux moteur, faux serveur, horloge injectée. Le moteur Node réel n'est lancé que
  par les tests marqués `intégration`, sur un compte factice.
- Pas de commit sans l'accord de Zed. Rien n'est déployé avant la tâche 2.6.

## Tâches

### Phase 0 — Fondations

| Id | But | Lire | Livrables | Test |
|---|---|---|---|---|
| 0.1 | Notes de décision : bibliothèque HTTP (serveur sans async si possible), crypto (RSA-PSS, AES-GCM, HMAC), TLS client, exception `unsafe`. | `docs/rust-pratiques.md`, `linux/http.mjs`, `linux/store.mjs` | `docs/decisions/*.md`, `DEPENDANCES.md` | relecture |
| 0.2 | Espace de travail vide, toolchain, `conduite-check` accepte `unsafe` dans une crate déclarée. | `apps/conduite-check` (Reseau) | `rust/Cargo.toml`, `rust-toolchain.toml`, crates vides, patch conduite-check + test | `cargo test` des deux dépôts |
| 0.3 | Figer le protocole : routes, corps, erreurs, en-têtes, reçus, avec exemples JSON. | `linux/http.mjs`, `linux/writes.mjs`, `remote-client/agent-tools.mjs`, `Remote/RemoteConnection.cs` | `docs/rust/protocole.md`, `rust/contrat/*.json` (cas dorés) | rejeu des cas dorés contre le serveur Node |

### Phase 1 — `vaultagent-core` (∥ entre elles)

| Id | But | Cahier de cas |
|---|---|---|
| 1.1 | Patch d'écriture (`name`, `folderId`, `notes`, champs de connexion, `addUris`, `removeUris`) et formes de requête, y compris cycle de vie. | `writes.test`, `write-lifecycle.test`, `fork/write-patch.mjs`, `write-checks.mjs` |
| 1.2 | Registre d'idempotence et reçus (trait de stockage), plan de vérification par relecture. | `write-ledger.mjs`, `write-lifecycle.mjs`, `writes.test` |
| 1.3 | Demandes d'association, appareils, clés d'approbateur, RSA-PSS. | `pair-requests.test`, `devices.mjs`, `keys.mjs` |
| 1.4 | Demandes de déverrouillage, validation UAC, vérification Hello. | `unlock.test`, `hello.test`, `unlock-requests.mjs`, `hello.mjs` |
| 1.5 | Coffres multiples, partage, droits des collections. | `multivault.test`, `vaults.mjs`, `sharing.mjs` |

### Phase 2 — Serveur Linux

| Id | But | Test |
|---|---|---|
| 2.1 | `vaultagent-engine` : enfant Node, protocole ligne à ligne, délais, redémarrage ; faux moteur pour les tests. | `engine-smoke` rejoué sur compte factice |
| 2.2 | Magasin chiffré, session, verrouillage, expiration. | `service.test` |
| 2.3 | Routes agent : `status`, `vaults`, `catalog`, `write-preview`, `write`, `sharing`, `share`, `seal`, `request-unlock`, `lock`. | cas dorés 0.3 |
| 2.4 | Routes admin et association, en-tête Authentik, fichiers du front, téléchargements. | `client.test`, `pair-requests.test` |
| 2.5 | Image Docker (moteur Node + binaire Rust), parité : mêmes requêtes sur le serveur Node et le serveur Rust, mêmes réponses. | script de parité en CI locale |
| 2.6 | Déploiement à côté de l'ancien (tag d'image distinct), bascule, retour arrière documenté. Accord de Zed requis. | vérification sur le coffre de test puis réel |

### Phase 3 — Client et CLI (∥ avec la phase 2 dès 0.3)

| Id | But | Test |
|---|---|---|
| 3.1 | Configuration privée, association par approbation web, client HTTPS (redirections refusées, délais). | faux serveur |
| 3.2 | Lecture : `status`, `vaults`, `catalog`, `preview`. | contrat JSON |
| 3.3 | Écriture : `create`, `update`, `delete`, `restore`, `folder`, valeurs par entrée standard. | contrat JSON, aucun secret en sortie |
| 3.4 | `unlock` avec attente bornée, `lock`. | faux serveur |
| 3.5 | `seal` et remplissage par enveloppe RSA-OAEP / AES-GCM. | aller-retour factice |
| 3.6 | Aide, codes de sortie, sorties versionnées, tests d'intégration du binaire. | `tests/` |
| 3.7 | Compilation Linux x86-64 et installation sur la VM agents. | commande réelle sur compte factice |

### Phase 4 — Windows (après 3.6)

| Id | But | Test |
|---|---|---|
| 4.1 | `vaultagent-win32` : identité de fenêtre, premier plan unique, saisie `SendInput` avec contrôle de cible. | fenêtre factice du test |
| 4.2 | Stockage DPAPI du jeton et de la clé d'approbateur. | aller-retour sur TowerZ |
| 4.3 | Clé de validation UAC (DPAPI machine, ACL), signature par processus élevé ; retrait de la clé sur preuve UAC (passage en mode carte). | enrôlement, signature et retrait réels |
| 4.4 | Windows Hello (preuve WebAuthn) quand disponible. | `hello-status` |
| 4.5 | Icône de barre, carte d'approbation, réglages, notifications ; réglage de validation carte ou UAC, une seule étape par demande. | essai à la main avec Zed |
| 4.6 | Installeur, remplacement du client .NET, retrait de .NET si plus utilisé. | installation sur TowerZ |

### Phase 5 — Bascule

| Id | But |
|---|---|
| 5.1 | Réécrire le skill `vault-agent` pour appeler `vaultagent` ; créer ses fiches de scénarios (`tests/scenarios/`, absentes aujourd'hui) puis les rejouer. |
| 5.2 | Retirer le serveur Node, `remote-client/`, le client WinForms, `vault.mjs` et le test de contrat `linux/test/contract.test.mjs` (seule exception JS, accordée par Zed le 15/11/2026 pour la tâche 0.3) ; mettre à jour README et `architecture.md`. |
| 5.3 | Publier la release, mettre à jour `stacks/maison/vaultagent` et `stacks/agents`. |

## Risques

- **Parité fine** : les 3 tests serveur déjà en échec (`client.test`, `hello.test`) sont corrigés en Node avant de
  servir de référence.
- **Windows Hello** est indisponible sur TowerZ : la tâche 4.4 peut se limiter à la vérification serveur.
- **Carte d'approbation en Win32** : le plus gros morceau d'interface ; si le coût dépasse l'estimation, on
  garde la validation par le web en secours, sans bloquer le reste.
- **Saisie sans intervention** (focus clavier, injection directe) reste un sujet à part : la CLI expose `focus` et
  `type`, l'injection par Chrome dédié aux agents est une tâche ultérieure, hors de ce plan.
