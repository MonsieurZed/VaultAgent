# Interface web : note de conception (tâche 7.1)

Rédigée le 17/11/2026 à la demande de Zed, qui trouve l'interface difficile à comprendre. Il demande quatre choses :
des erreurs lisibles, des libellés clairs, des onglets réorganisés et aucun défilement. À relire par Zed avant les
tâches 7.2 à 7.4.

## Constat

L'interface actuelle (2.4b) est rendue par `vaultagent-server/src/front_*.rs`, sans JavaScript.

- Le bandeau d'accueil (titre, accroche, paragraphe) occupe à lui seul la moitié de l'écran ; l'onglet « Clients »
  empile ensuite le guide d'installation, les demandes, la création d'un code et la liste des appareils.
- Les demandes de déverrouillage s'affichent au-dessus des onglets ; les demandes d'ajout d'appareil sont enfouies
  dans « Clients ».
- Le vocabulaire vient de Node : « client », « association », « Autoriser les clients », « approbateur »,
  « enveloppe chiffrée ».
- Une erreur de contrôle (origine, taille, route) renvoie le JSON du protocole au navigateur.
- « Révoquer » agit sans confirmation.
- La page se recharge toutes les 5 s sur « Clients », ce qui efface un code 2FA en cours de saisie.

## Principes

1. Tout tient dans une fenêtre de 1366 × 768 sans défilement ; au-delà, les listes sont paginées et rien ne s'empile.
2. Une seule page principale, sans onglets (décision de Zed du 17/11/2026) ; ce qui attend Zed vient en premier.
3. Un seul mot par notion, celui de Windows quand il existe.
4. Une erreur dit ce qui s'est passé et quoi faire, en une phrase, sur la page.
5. Toute action irréversible se confirme sur la ligne même.
6. La sécurité ne bouge pas : aucun script, même CSP, même contrôle d'origine, mêmes routes JSON.

## Mise en page

Une grille sur toute la hauteur, sans pied de page (la version passe dans « Aide »). De haut en bas :

1. **Barre d'état** (44 px) : nom, pastille d'état (« Verrouillé », « Déverrouillé jusqu'à 08:42 », « À configurer »),
   bouton « Déverrouiller » (champ 2FA à côté) ou « Verrouiller », langue (liste courte), « Aide ».
2. **Ligne du coffre** (32 px) : « zed@… sur coffre.lan.zed.cx · verrouillage après 15 min · Modifier ».
3. **À valider**, affichée seulement si une demande attend : demandes de déverrouillage (appareil, heure, échéance,
   2FA, « Déverrouiller », « Refuser ») et demandes d'ajout d'appareil (nom, code à comparer, rôle demandé,
   « Ajouter », « Refuser »).
4. **Appareils** : une ligne de 44 px par appareil, avec le nom, le rôle (« validateur » avec son mode UAC, carte ou
   Hello, ou « agent »), la date d'ajout, la case « sans demander » pour un agent et « Retirer » avec
   confirmation. Le titre porte « Ajouter un appareil ». La liste montre 8 appareils, moins quand des demandes
   attendent ; au-delà, elle est paginée (`?page=`).

Budget à 1366 × 768 (environ 650 px utiles) : 44 + 32 + jusqu'à 3 demandes (160) + 8 appareils (380).

## Pages secondaires

| Page | Contenu | Remplace |
|---|---|---|
| `/?view=vault` | Adresse, e-mail, mot de passe maître, 2FA, verrouillage automatique et délai, « Vérifier et enregistrer », avec l'avertissement « changer de compte retire tous les appareils » à côté du bouton. Saisie une fois, ouverte par « Modifier » ; c'est aussi la page d'arrivée tant que le coffre est « À configurer ». | onglet « Compte » |
| `/?view=add` | Nom de l'appareil, création du code d'ajout, liens de téléchargement des programmes Windows et Linux. | création de code et guide de « Clients » |
| `/?view=help` | Ce que protège le remplissage, comment ajouter un appareil, version du serveur. | « Fonctionnement », pied de page |

Chaque page secondaire tient aussi sans défilement et porte un lien « Retour ».

Le mode de validation (carte, UAC) reste réglé depuis l'icône de barre de TowerZ, car il demande une preuve UAC.
L'interface web ne fait que l'afficher.

## Vocabulaire

| Avant | Après |
|---|---|
| client | appareil |
| association, code d'association | ajout d'appareil, code d'ajout |
| approbateur | validateur |
| Autoriser les clients | Déverrouiller |
| Coffre disponible pour les clients | Déverrouillé jusqu'à HH:MM |
| Autoriser cette demande | Déverrouiller |
| Toujours accepter les demandes de ce client | Sans demander |
| Révoquer | Retirer, puis « Confirmer le retrait » |
| Compte | Coffre, page « Modifier » |
| Clients, Fonctionnement (onglets) | (retirés : page unique) |
| enveloppe chiffrée | (retiré de l'interface) |

Les traductions (`i18n_*.rs`) suivent la même table ; une clé absente reste en français.

## Erreurs

- **Formulaires** (`POST /admin/*` venant du navigateur) : toute erreur, y compris `origin-refused`,
  `request-size` et `route-refused`, devient une redirection `303` vers l'onglet. Elle porte un code de message
  et aucune action n'est faite. Exemple : « La page n'était plus à jour, l'action n'a pas été faite. Réessaie. »
  Le message prend la place de la ligne du coffre, pour ne pas pousser la liste.
- **Pages** (`GET` avec `Sec-Fetch-Mode: navigate` ou `Accept: text/html`) : page d'erreur HTML d'une phrase avec un
  lien de retour.
- **Routes JSON** (agents, clients) : contrat inchangé, cas dorés inchangés.
- Chaque code d'erreur du protocole a son message ; un code inconnu donne « Action refusée par le serveur. ».

## Rafraîchissement

La page principale se recharge toutes les 10 s seulement quand aucune demande n'attend, pour faire apparaître une
nouvelle demande. Dès qu'une demande attend, elle ne se recharge plus, pour ne pas effacer une saisie ; « Actualiser »
reste disponible dans la barre. Les pages secondaires ne se rechargent pas.

## Hors périmètre

- Dernière utilisation d'un appareil : le serveur ne la stocke pas. À ajouter au besoin avec la phase 6.
- Thème sombre : il suit `prefers-color-scheme` si la feuille de style le permet sans grossir, sinon il est reporté.

## Découpage

- 7.2 : erreurs (redirection des formulaires, page HTML, table des messages).
- 7.3 : page unique (barre d'état, ligne du coffre, demandes, appareils), pages secondaires et vocabulaire ;
  retrait de `front_tab.rs`.
- 7.4 : grille sans défilement, pagination, confirmation du retrait et rafraîchissement.

Chaque tâche a ses tests de rendu (présence et absence des éléments, en-têtes) et les cas dorés du front mis à jour.
L'essai final est fait par Zed sur TowerZ à 1366 × 768 et à sa résolution habituelle.
