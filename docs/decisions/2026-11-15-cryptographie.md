# Cryptographie (15/11/2026)

Tâche 0.1 du [plan de réécriture](../rust/plan.md).

## Contexte

Opérations faites aujourd'hui par `node:crypto` :

| Opération | Où (Node) | Côté |
|---|---|---|
| AES-256-GCM, nonce 96 bits, clé machine de 32 octets | `store.mjs` (`protect`, `unprotect`) | serveur |
| RSA-OAEP SHA-256 + AES-256-GCM (enveloppe `seal`) | `session.mjs` (`seal`) | chiffrement serveur, déchiffrement client |
| Vérification RSA-PSS SHA-256, sel 32 octets | `keys.mjs`, `unlock-requests.mjs` | serveur ; signature par le client |
| Vérification WebAuthn : ECDSA P-256 (DER) ou RSA PKCS#1 v1.5, SHA-256 | `hello.mjs` | serveur |
| HMAC-SHA-256 (empreinte d'idempotence et de partage, clé machine) | `write-ledger.mjs`, `sharing.mjs` | serveur |
| SHA-256 (`digest`) | `store.mjs`, `hello.mjs` | serveur |
| Comparaison à temps constant | `store.mjs` (`same`), `hello.mjs` | serveur |
| Aléa (clés, défis, jetons, identifiants) | partout | serveur, client |

Le format des données sur disque (`machine.key`, `{nonce, tag, data}` en base64) et des messages doit rester
identique : le serveur Rust reprend le dossier `/data` du serveur Node sans migration.

## Choix

Famille **RustCrypto**, en Rust pur :

| Besoin | Crate |
|---|---|
| AES-256-GCM | `aes-gcm` |
| RSA-OAEP, RSA-PSS, RSA PKCS#1 v1.5, lecture SPKI DER | `rsa` (fonctions `sha2`, `std`) |
| ECDSA P-256, signature DER | `p256` (fonction `ecdsa`) |
| HMAC-SHA-256 | `hmac` |
| SHA-256 | `sha2` |
| Comparaison à temps constant | `subtle` |
| Aléa du système | `getrandom` (et `rand_core` avec `getrandom`, exigé par `rsa`) |
| Effacement des tampons sensibles | `zeroize` |
| Base64 standard et `base64url` | `base64` |

Côté Windows, les clés privées d'approbateur et d'élévation restent dans NCrypt (tâches 4.2, 4.3) : la signature
RSA-PSS se fait par l'API Windows dans `vaultagent-win32`, pas par `rsa`.

## Raisons

- Rust pur : aucune compilation C, donc la même chaîne GNU sous Windows et Linux, et une image Docker simple.
- `ring` n'offre ni RSA-OAEP ni la lecture de clés SPKI arbitraires pour la vérification PSS avec sel fixé ;
  `aws-lc-rs` exige CMake et un compilateur C.
- `openssl` lierait une bibliothèque système et compliquerait la compilation Windows.
- Les crates RustCrypto partagent les mêmes traits (`digest`, `signature`) : une seule famille à suivre.

## Conséquences

- **Avis RUSTSEC-2023-0071 (Marvin)** sur `rsa` : fuite temporelle du **déchiffrement** RSA. Le serveur ne fait
  que chiffrer (OAEP) et vérifier (PSS, PKCS#1) avec des clés publiques : il n'est pas concerné. Le client
  déchiffre l'enveloppe `seal` en local, une fois par remplissage, sans oracle accessible à distance. L'avis est
  ignoré dans `deny.toml` avec ce motif ; il est réexaminé à chaque entretien mensuel et dès qu'une version
  corrigée de `rsa` sort.
- Les tests d'interopérabilité vérifient, contre des vecteurs produits par le serveur Node (cas dorés 0.3) :
  déchiffrement d'un `{nonce, tag, data}`, ouverture d'une enveloppe `seal`, vérification d'une signature PSS
  et d'une preuve WebAuthn factices, empreinte HMAC d'une écriture.
- Toute valeur secrète en clair (mot de passe, TOTP, clé AES) vit dans un type qui s'efface à la destruction
  (`zeroize::Zeroizing`).
