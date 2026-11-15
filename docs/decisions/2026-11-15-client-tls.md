# Client HTTPS (15/11/2026)

Tâche 0.1 du [plan de réécriture](../rust/plan.md).

## Contexte

Les clients actuels parlent au serveur en HTTPS seulement :

- `remote-client/agent-tools.mjs` : adresse `https://` sans chemin, utilisateur, requête ni fragment
  (`validServer`), confiance du système plus `NODE_EXTRA_CA_CERTS` dans les tests ;
- `Remote/RemoteConnection.cs` : `HttpClient` sans redirection (`AllowAutoRedirect = false`), délai de 110 s,
  magasin de certificats de Windows.

Le serveur public est derrière Caddy (certificat d'une autorité publique) ; un serveur interne peut utiliser
l'autorité privée du homelab, installée dans le magasin du système.

## Choix

**`ureq` 3**, synchrone, `default-features = false`, fonctions `rustls` et `platform-verifier` :

- TLS par `rustls` (fournisseur `ring`) ;
- vérification des certificats par le magasin du système (`rustls-platform-verifier` : Windows, puis le magasin
  du système sous Linux), comme le client .NET ;
- redirections désactivées (`max_redirects(0)`), toute réponse 3xx est une erreur ;
- délai global de 110 s par requête, 10 s pour la connexion ;
- pas de `gzip`, de cookies, de proxy ni de `json` (le JSON passe par `serde_json`).

L'adresse du serveur est validée avant tout appel, avec les règles de `validServer`.

## Raisons

- La CLI est un outil en ligne de commande : synchrone, comme le veut `docs/rust-pratiques.md`.
- `ureq` est petit et déjà utilisé par le hub (en test) ; `reqwest` tirerait `tokio` et `hyper`.
- Le magasin du système suit l'autorité privée du homelab sans copie de certificat dans la configuration.
- `ring` demande un compilateur C, présent sur TowerZ (MinGW, chaîne GNU) et dans l'image de compilation Linux.

## Conséquences

- Le transport est derrière un trait (`Transport::post(path, body) -> Response`) dans `vaultagent-client` :
  les tests utilisent un faux serveur en mémoire, aucun réseau.
- Un test d'intégration HTTPS sur `127.0.0.1` avec un certificat jetable reste possible plus tard, marqué
  `intégration`, hors de la suite par défaut.
- Si `ring` pose un jour problème de compilation, `ureq` accepte `native-tls` (SChannel sous Windows) sans
  changer le code appelant.
