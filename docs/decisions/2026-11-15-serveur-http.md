# Serveur HTTP du serveur Rust (15/11/2026)

Tâche 0.1 du [plan de réécriture](../rust/plan.md).

## Contexte

Le serveur Node (`linux/http.mjs`) écoute en HTTP simple sur le port 8080 ; le TLS est terminé par le proxy
privé (Caddy), dont l'adresse est la seule acceptée (`VAULTAGENT_TRUSTED_PROXY`, refus `proxy-required`).
La charge est minuscule : quelques agents et un administrateur. Ce dont le serveur a besoin :

- lire méthode, chemin, chaîne de requête et en-têtes (`authorization`, `origin`, `content-type`,
  `x-authentik-username`) ;
- connaître l'adresse distante du pair TCP, pour le contrôle du proxy de confiance ;
- lire un corps JSON borné à 24 576 octets ;
- répondre en JSON, servir trois fichiers du front et les archives de téléchargement en flux ;
- délais : en-têtes 10 s, requête 120 s (`server.headersTimeout`, `server.requestTimeout`), 40 en-têtes au plus.

Une écriture peut attendre le moteur Node jusqu'à 90 s : les requêtes ne doivent pas se bloquer entre elles.

`docs/rust-pratiques.md` : « asynchrone seulement dans les services qui en ont besoin ».

## Choix

**`tiny_http` 0.12**, sans fonction optionnelle (pas de `ssl-*` : le TLS reste au proxy). Un fil d'écoute reçoit
les requêtes et les confie à un petit ensemble fixe de fils de travail (8, réglable), écrit avec la bibliothèque
standard (`std::thread`, `std::sync::mpsc`). Le routage est une fonction pure
`route(&Request) -> Response` sur des types internes à `vaultagent-server`, indépendants de `tiny_http` ;
l'adaptateur `tiny_http` vit dans un module de bordure.

## Raisons

- Synchrone : pas de `tokio`, pas de couleur `async` dans la logique ; le moteur Node est déjà un processus à
  part, les attentes longues bloquent un fil de travail, pas le serveur.
- `tiny_http` donne l'adresse distante (`remote_addr`), les en-têtes bruts et le corps en flux ; il est petit
  (quelques dépendances, aucune en C) et largement utilisé (`rouille` est construit dessus).
- `axum` (choisi pour Trackerr et le hub) imposerait `tokio`, `tower` et `hyper` pour un service à quelques
  requêtes par minute.
- Un serveur HTTP/1.1 écrit à la main : analyse des en-têtes, `Transfer-Encoding`, `Expect: 100-continue`,
  connexions persistantes ; trop de surface pour un service qui garde des secrets.

## Conséquences

- `tiny_http` n'a pas de délai d'en-têtes propre : le module de bordure pose `set_read_timeout` /
  `set_write_timeout` sur la connexion et coupe toute requête dont la lecture dépasse les bornes Node ; un test
  le vérifie avec un client lent factice.
- Le nombre d'en-têtes (40) et la taille du corps (24 576 octets) sont contrôlés par le serveur lui-même, avant
  toute lecture JSON.
- Les tests de routes appellent `route` sans socket ; seul le test de l'adaptateur ouvre `127.0.0.1:0`.
- Si un jour il faut HTTP/2 ou des milliers de connexions, l'adaptateur se remplace sans toucher au routage.
