# Clients Windows et Linux — preview

Le serveur conserve le moteur Bitwarden et le mot de passe du coffre chiffre au repos.
Le client recoit une enveloppe RSA-OAEP/AES-GCM, la dechiffre dans le helper et remplit
le champ sans presse-papiers. Aucune valeur n'est renvoyee au modele dans le flux normal.
Un agent avec du code arbitraire ou le controle complet du navigateur peut encore lire
les champs. Ce mecanisme ne constitue pas une isolation contre cet agent.

## Enregistrer et gérer les fiches — preview.2

Le helper expose `writePreview(itemId?)`, `createFolder(operationId,name)` et
`saveFromSource({operationId,action,metadata,source,itemId?,expectedRevisionDate?})`.
Les secrets restent dans une fonction source locale puis traversent stdin/HTTPS ; les reçus
contiennent seulement UUID, action, révision et vérification. Connexions et notes sécurisées
sont prises en charge. Les mises à jour ciblées préservent les passkeys et les champs non visés.
Guide et exemples : [écriture sans lecture](https://github.com/MonsieurZed/VaultAgent/blob/main/skills/vault-agent/references/write.md).

Le serveur doit être déverrouillé et le client associé. Aucune lecture brute ni commande de
suppression n'est ajoutée. Le service Windows autonome 2.x reste inchangé.
Conserver une UUID par opération : répétition identique sans doublon, contenu différent refusé.
Après `write-indeterminate`, arrêter les répétitions et réconcilier les métadonnées ; une panne
peut avoir laissé une écriture effective. Ne pas changer l'UUID pour contourner cette protection.

## Windows

L'installateur ajoute le bouclier en arriere-plan, le demarrage automatique et la desinstallation
pour le compte Windows courant. L'association existante est conservee. Il ne cree pas de service
SYSTEM : l'agent fonctionne dans la session utilisateur pour pouvoir remplir les champs.
Une association au serveur suffit ; les champs restent vides avec un exemple d'adresse.

Option portable : extraire le ZIP dans un dossier du compte Windows. Lancer `VaultAgent-Client.exe`.
Saisir l'adresse HTTPS du serveur et le code a usage unique cree dans son onglet Clients.
Ne jamais saisir le mot de passe maitre dans le client, ni transmettre le code dans un chat.
Le jeton est protege par DPAPI pour le compte Windows courant. Le programme fonctionne sans
le service HomelabVaultAgent et sans l'installer. Le mode Windows autonome existant reste distinct.

```javascript
var remote = await import('file:///C:/CHEMIN/remote-agent-tools.mjs');
nodeRepl.write(await remote.catalog('nom du site'));
nodeRepl.write(await remote.fillBrowser({
  locator: tab.playwright.getByLabel('Mot de passe', {exact:true}),
  itemId: 'UUID choisi dans le catalogue', field: 'password',
  origin: 'https://site.example'
}));
```

Champs : username, password, totp. L'origine HTTPS exacte doit figurer dans les URL de la fiche.
Verifier la page cible avant le remplissage. Ne jamais lire le DOM apres remplissage, inspecter
les variables du helper ou journaliser le contenu des champs.
`typeWindows` permet aussi la saisie dans le champ Windows actuellement selectionne.
`process` (executable, ex. `chrome`) et `title` (fragment du titre de la page) sont obligatoires :
la saisie est refusee si la fenetre active ne correspond pas, ou change pendant la frappe.
Un titre generique (`Chrome`, `New Tab`) est rejete. `foregroundWindow()` renvoie le processus et
le titre actuels pour les choisir. En cas de refus, l'erreur nomme la fenetre active et la cible.

## Linux

Le TAR Linux contient Node 22. Extraire le paquet, puis dans un terminal local interactif :

```sh
./vaultagent-client pair https://vaultagent.lan.zed.cx
./vaultagent-client status
./vaultagent-client catalog 'nom du site'
```

Le code est saisi masque. La configuration est dans `~/.config/vaultagent/client.json`,
permissions 0600. Ne pas lire ce fichier depuis un outil d'agent et ne pas le versionner.
Importer `remote-client/agent-tools.mjs` pour remplir un locator Playwright sur Linux.
Les clients ne peuvent ni configurer, ni deverrouiller le coffre. L'administrateur l'autorise
depuis l'interface web, pour la duree configuree. Une revocation verrouille aussi le coffre.
Les passkeys du fournisseur Windows experimental ne sont pas portees vers Linux dans cette preview.
## Demander l'ouverture du coffre

`await remote.requestUnlock()` enregistre une demande sans ouvrir le coffre.
`await remote.unlock({waitSeconds:60})` demande puis attend l'accord ; reprendre seulement
si `unlocked:true`. Les états `denied`, `failed` et `expired` imposent l'arrêt sans relance.
Le PC Windows approbateur affiche Oui/Non avec Toujours accepter ; aucun bouton d'approbation
ne doit être piloté par l'agent. L'acceptation automatique est conservée par client sur Linux
et fonctionne sans le PC Windows. Elle peut être retirée dans Clients ou par le menu du bouclier.
Linux : `vaultagent-client unlock [secondes]`. Windows : `VaultAgent-Client.exe request-unlock`.
Ces commandes ne prolongent pas la session ; les demandes expirent après cinq minutes.
## Sélection du coffre — preview.4

`await remote.vaults()` retourne seulement les coffres autorisés. Passer `{vaultId:'UUID'}`
aux fonctions `status`, `catalog`, `writePreview`, `sharing`, `share` et `createFolder`, ou
`vaultId` dans `saveFromSource`, `fillBrowser`, `typeWindows`, `unlock`.
Si le client accède à plusieurs coffres, aucune sélection implicite n'est acceptée.
CLI : `vaults`, puis `status --vault UUID`, `catalog nom --vault UUID`, `sharing --vault UUID`.
Le bouclier surveille les coffres autorisés, et indique le nom du coffre dans la confirmation.
Les transferts natifs entre compte personnel et organisation utilisent `share` avec prévisualisation,
révision et confirmation du changement de propriété. Lire le guide du skill avant d'appliquer.
## Windows Hello — preview.5

Le menu Activer Windows Hello du bouclier configure la preuve verifiee par Linux pour les
confirmations manuelles. `hello-status` ne lance pas de verification ; `hello-setup` ouvre
la configuration interactive. Le PIN reste dans Windows. Annulation/indisponibilite refusent
l'accord, avec Aucun secours automatique. Les politiques Toujours accepter restent actives.
Voir [le guide](https://github.com/MonsieurZed/VaultAgent/blob/main/docs/remote-hello.md) pour les conditions et limites.
