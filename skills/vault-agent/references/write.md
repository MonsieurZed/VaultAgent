# Enregistrer et gérer les fiches sans lecture — serveur 3

Le client Windows associé est installé dans `%LOCALAPPDATA%\Programs\VaultAgentClient`.
Importer son `remote-agent-tools.mjs` dans un runtime Node local. Sous Linux, importer
`remote-client/agent-tools.mjs` du TAR installé et utiliser le Node inclus si nécessaire.
Ne pas lire le fichier d'association, le jeton, les profils du coffre ou les secrets locaux.
Vérifier `status()` ; si le coffre est verrouillé, utiliser [la demande de déverrouillage](unlock.md)
avec le client 3.0.0-preview.3 ou ultérieur. Laisser Zed approuver ; sur une version antérieure,
il doit autoriser les clients dans l'interface. Ne pas changer son timeout ni contourner par le service autonome.

## Choix de la fiche et de l'opération

`writePreview()` renvoie seulement les dossiers et les métadonnées des connexions et notes
actives, personnelles ou communes. Avec preview.4, choisir d'abord `vaultId` selon le
[guide multi-coffre](multi-vault.md). `writePreview(itemId,scope)` renvoie notamment
`{id,name,type,folderId,revisionDate,organizationId,collectionIds}` et, pour une fiche seule,
`deleted`, `uris` (les adresses enregistrées, non secrètes) et `has:{username,password,totp,notes}`
(présence des champs, jamais leur valeur). La liste contient aussi `trash` : les fiches en corbeille.
Vérifier la cible avec l'utilisateur et conserver la révision exacte avant une modification.
Une fiche archivée ou supprimée est refusée. Une fiche commune exige son `organizationId`
explicite pour l'édition, la corbeille et la restauration, avec les droits natifs de la collection. Aucune fusion
n'est proposée et aucune suppression définitive : la corbeille reste récupérable.

Créer une UUID publique `operationId` avant chaque opération et la conserver avec le reçu.
Réutiliser exactement cette UUID et la même requête lors d'une répétition ; une UUID réutilisée
avec un contenu différent est refusée. Un succès répété renvoie le même reçu sans réécrire.
Après une erreur indéterminée, arrêter : ne pas changer d'UUID pour forcer un nouvel essai.
La fiche a peut-être été créée ; une inspection des métadonnées et une réconciliation humaine
peuvent être nécessaires. Ne pas régénérer, changer ou faire tourner un secret existant pour sauver.

## Source des valeurs

`saveFromSource` appelle une fonction `source()` dans le même processus que le helper.
Elle retourne uniquement les champs `username`, `password`, `totp`, `notes` à enregistrer.
Le helper les transmet au service par HTTPS, ou par stdin au client Windows puis HTTPS.
Seul le reçu sort : `{id,operationId,action,verified,revisionDate}` ; jamais le secret ni son hash.
Ne pas écrire ces valeurs dans les arguments, un fichier temporaire, les logs ou la sortie outil.
Ne pas afficher le retour de la source ni instrumenter le helper. Les exceptions de la source sont masquées.

La source doit disposer d'un accès autorisé qui garde les valeurs dans le runtime : par exemple
une API de gestion appelée par un module local sans logs. **Cette méthode n'installe pas de
connecteur Omada universel.** Si les outils ne peuvent fournir ce flux sans remonter la valeur
au modèle, expliquer cette limite et faire saisir la valeur dans une interface approuvée ;
ne pas demander le secret dans le chat. Le contrôle complet du processus permet toujours
de lire ses données : il ne s'agit pas d'une isolation contre un agent malveillant.

## Exemples de structure, sans valeur préremplie

Les noms, origines et UUID ci-dessous sont uniquement des exemples. `sourceAutorisee` représente
un module adapté à la source réelle, pas une API livrée automatiquement par VaultAgent.

```javascript
var operationId = crypto.randomUUID(); // conserver cette UUID publique
var receipt = await remote.saveFromSource({
  operationId, action: 'create',
  metadata: { type: 1, name: 'Exemple de compte',
    login: { uris: [{ uri: 'https://service.example' }] } },
  source: async () => ({ password: await sourceAutorisee.lireDansCeProcessus() })
});
nodeRepl.write(receipt);
```

Pour une note sécurisée : `metadata:{type:2,name:'Exemple de note'}` et une source
retournant `{notes: valeur}`. Pour un dossier : `createFolder(operationId,'Exemple de dossier')`.
Le dossier existant est réutilisé si son nom exact est unique ; le reçu contient son UUID.
Utiliser cet UUID dans `metadata.folderId`, ou `null` pour retirer une fiche d'un dossier.

Modifier une fiche : `action:'update', itemId, expectedRevisionDate` obtenus par prévisualisation.
`metadata` peut contenir `name` et `folderId`; la source peut fournir `notes`, `username`,
`password` ou la **graine** `totp` (pas un code éphémère). Pour un classement seul, `source:async()=>({})`.
Seuls les champs fournis changent. Les passkeys, champs personnalisés, pièces jointes et autres
données restent dans le service.

Adresses (`login.uris`) : `metadata:{addUris:['https://adresse.exacte']}` ajoute des adresses HTTPS
(sans identifiants, requête ni fragment, vingt au plus, doublons ignorés). `metadata:{removeUris:['adresse telle que dans uris']}`
retire des adresses, quel que soit leur schéma ; chacune doit exister, sinon la requête est refusée. Les deux se combinent
dans une même écriture, le retrait passant avant l'ajout : c'est ainsi qu'on corrige ou remplace une adresse.

## Corbeille, restauration et dossiers

Ces opérations n'ont aucun secret : elles s'appellent directement, avec une UUID `operationId` et la
révision lue par `writePreview(itemId)`.

- `deleteItem({operationId,itemId,expectedRevisionDate,organizationId?})` met la fiche en corbeille ; le reçu donne la nouvelle `revisionDate`.
- `restoreItem({operationId,itemId,expectedRevisionDate,organizationId?})` la sort de la corbeille (révision lue sur la fiche supprimée).
- `renameFolder({operationId,folderId,name})` renomme un dossier ; un nom déjà pris est refusé.
- `deleteFolder({operationId,folderId})` supprime le dossier ; ses fiches restent dans le coffre, sans dossier.

Chaque opération relit le coffre pour se vérifier, renvoie le même reçu si elle est répétée à l'identique, et suit les
règles de reprise ci-dessus. La suppression définitive d'une fiche n'est pas proposée : vider la corbeille reste un geste de Zed.
La vérification relit le coffre en interne ; elle ne valide pas le fonctionnement du secret sur le site.
