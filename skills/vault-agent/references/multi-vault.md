# Plusieurs coffres et fiches communes — preview.4

Le helper du client associé expose `vaults()`. Il retourne les UUID, noms et états des seuls
coffres accessibles au client, sans comptes ni secrets. Choisir le coffre demandé par Zed et
utiliser son UUID dans toutes les opérations. Si plusieurs choix conviennent, demander lequel.
Ne pas déduire l'accès à un coffre de son nom ; ne pas passer par Admin si Agent est refusé.

```javascript
nodeRepl.write(await remote.vaults());
var scope = {vaultId: 'UUID-retourne-par-vaults'}; // ou default si ce coffre est choisi
nodeRepl.write(await remote.status(scope));
nodeRepl.write(await remote.catalog('Exemple', scope));
nodeRepl.write(await remote.sharing(scope));
```

`remote` est le helper installé du client, décrit dans write.md et unlock.md. Il ne s'agit pas
d'une nouvelle CLI Bitwarden. Un seul coffre accessible peut être sélectionné implicitement ;
plusieurs exigent `vaultId`. Linux/Windows : `vaults`, puis `status --vault UUID` ou
`catalog Exemple --vault UUID`. Le helper gère le jeton ; ne pas lire son fichier d'association.

Les déverrouillages et autorisations automatiques sont séparés : `unlock({...scope,waitSeconds:60})`.
Ne pas activer de politique, accorder l'accès à un client, créer des comptes ou enregistrer un PC
approbateur pour contourner un refus. La configuration privée se fait dans l'interface par Zed.
Pour limiter un agent au coffre Agent, utiliser un client associé uniquement à ce coffre.
Un client auquel Zed accorde aussi Admin dispose réellement de cet accès ; le nom du client
ou le choix habituel du coffre ne constitue pas une restriction de permissions.

## Créer et modifier une fiche commune

Les deux comptes doivent être membres confirmés de la même organisation sur le même serveur
Vaultwarden, avec accès à la collection commune. `sharing(scope)` retourne les organisations,
collections et `canEdit`, sans valeurs. Les rôles, invitations et révocations se gèrent dans
Vaultwarden ; ne pas inviter de membres ou modifier leurs droits sans demande explicite.

Créer une connexion : `saveFromSource({...scope,operationId,action:'create',
metadata:{type:1,name,organizationId,collectionIds:[collectionId],login:{uris:[{uri}]}},source})`.
Créer une note : `type:2` et la source retourne `{notes:...}`. Les valeurs viennent de la fonction
source autorisée, sans arguments/logs/fichiers temporaires ni retour dans le chat.

Avant une modification, `writePreview(itemId,scope)` fournit la révision et l'organisation.
Appeler `saveFromSource({...scope,operationId,action:'update',itemId,expectedRevisionDate,
organizationId,metadata:{name},source})`. Les champs fournis sont modifiés en place ; les autres
restent conservés. Le SDK refuse un membre en lecture seule. L'autre compte voit la même fiche
et ses modifications après synchronisation. Aucune copie entre deux serveurs n'est réalisée.

## Partager une fiche personnelle existante

Ce partage **transfère sa propriété à l'organisation** ; les membres autorisés de la collection
y auront accès. Préparer la destination et obtenir le choix de l'utilisateur avant de transférer.
Un changement de propriété ne doit pas être présenté comme une simple étiquette ou sauvegarde.

```javascript
var plan = {action:'preview',itemId,organizationId,collectionIds:[collectionId]};
var preview = await remote.share(plan, scope);
nodeRepl.write(preview); // titre, destination et révision uniquement
// Après autorisation de cette destination et de ce transfert :
nodeRepl.write(await remote.share({...plan,action:'apply',operationId,
  expectedRevisionDate:preview.expectedRevisionDate,confirmOwnershipTransfer:true}, scope));
```

Conserver l'UUID d'opération et la requête exacte. Après résultat indéterminé, arrêter sans
nouvelle UUID ni réessai aveugle. Les fiches avec pièces jointes ne sont pas transférées par
cette commande. Elle ne fusionne, ne supprime et ne réattribue pas les permissions d'une fiche
déjà commune. Créer une collection si demandé : `share({action:'create-collection',operationId,
organizationId,name},scope)`. Les permissions natives restent appliquées ; le reçu est sans secret.
