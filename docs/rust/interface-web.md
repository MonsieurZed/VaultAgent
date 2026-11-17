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

1. Tout tient dans une fenêtre de 1280 × 720 sans défilement (décision du 17/11/2026 : blocs plein écran 16/9) ; les listes sont paginées dans leur bloc.
2. Une seule page principale, sans onglets (décision de Zed du 17/11/2026) ; ce qui attend Zed vient en premier.
3. Un seul mot par notion, celui de Windows quand il existe.
4. Une erreur dit ce qui s'est passé et quoi faire, en une phrase, sur la page.
5. Toute action irréversible se confirme sur la ligne même.
6. La sécurité ne bouge pas : aucun script, même CSP, même contrôle d'origine, mêmes routes JSON.

## Mise en page

Décision de Zed du 17/11/2026 : la page en colonne étroite de la phase 7 est remplacée par des **blocs plein écran
16/9**. La page tient dans la fenêtre à partir de 1280 × 720, sans défilement, sans largeur maximale.

**Barre** (40 px) : la marque VaultAgent (bouclier violet), le message de la dernière action en pastille (il ne pousse
pas la grille), et à droite les langues, « Actualiser » et « Aide ». Elle ne porte ni l'état ni les boutons du coffre.

**Grille** sous la barre : 3 colonnes égales, 2 rangées (haute 45 %, basse 55 %), marge et écart de 12 px, qui remplit
toute la fenêtre restante (`main{min-height:0;padding:12px}`, jamais de marge automatique, sinon la grille flotte au
milieu).

| Bloc | Place | Contenu |
|---|---|---|
| **Coffre** (`b-vault`) | colonne 1, rangée haute | État en grand (« Verrouillé » orange, « Déverrouillé jusqu'à 09:15 » vert, « À configurer »), champ 2FA et « Déverrouiller » (bouton plein) ou « Verrouiller », en bas le résumé « e-mail sur hôte · verrouillage après N min » et « Modifier ». |
| **À valider** (`b-pending`) | colonnes 2-3, rangée haute | Demandes de déverrouillage et d'ajout, compteur en pastille dans le titre. Vide : « Rien à valider » et « La page se met à jour seule. », centrés. |
| **Appareils** (`b-devices`) | colonnes 1-2, rangée basse | Une ligne par appareil (rôle, date d'ajout, « Sans demander » pour un agent, « Retirer » avec confirmation sur la ligne), compteur dans le titre, 6 lignes par page puis pagination dans le bloc (`?page=`). |
| **Ajouter un appareil** (`b-add`) | colonne 3, rangée basse | Nom de l'appareil et « Créer un code d'ajout » dans le bloc ; en bas, les téléchargements Windows et Linux avec leur SHA256. |

**Mode « Modifier »** (`/?view=vault`) : le formulaire du coffre prend la place du contenu du bloc Coffre, qui s'étend
sur les deux rangées de la colonne 1 (classe `blocks edit`) ; « Appareils » passe en colonne 2 seule de la rangée
basse, les autres blocs restent visibles. « Annuler » revient à `/`. Tant que le coffre n'est pas configuré, la page
principale s'ouvre d'office dans ce mode, sans « Annuler » ; le bloc affiche alors la pastille « À configurer ».

**Aide** (`/?view=help`) : page à part, un seul bloc large, lien « Retour » dans la barre. La page du code d'ajout,
servie directement après « Créer un code d'ajout », reste aussi une page à part avec « Retour ». `/?view=add` n'existe
plus : une valeur inconnue de `view` rend la page principale.

**Style** : fond de page légèrement teinté, blocs sur fond de carte (bordure fine, coins de 12 px), police de 14 px,
variables de couleur sur `:root` pour les thèmes clair et sombre (`prefers-color-scheme`). Un seul bouton plein (couleur
d'accent) par bloc, les autres en contour ; « Retirer » et « Confirmer le retrait » en rouge. Couleurs d'état : vert
(déverrouillé), orange (verrouillé), rouge (danger). Feuille de moins de 8 Ko.

**Responsive** : sous 1000 px de large, les blocs passent en une seule colonne et la page défile.

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
reste disponible dans la barre. Elle ne se recharge pas non plus en mode « Modifier » (une saisie serait perdue), quand un message s'affiche ou quand
une confirmation de retrait est ouverte, ni sur l'aide.

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
