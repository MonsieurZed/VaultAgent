# Clients Windows et Linux — preview

Le serveur conserve le moteur Bitwarden et le mot de passe du coffre chiffre au repos.
Le client recoit une enveloppe RSA-OAEP/AES-GCM, la dechiffre dans le helper et remplit
le champ sans presse-papiers. Aucune valeur n'est renvoyee au modele dans le flux normal.
Un agent avec du code arbitraire ou le controle complet du navigateur peut encore lire
les champs. Ce mecanisme ne constitue pas une isolation contre cet agent.

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
`typeWindows` permet aussi la saisie dans le champ Windows actuellement selectionne :
la fenetre active doit rester identique jusqu'a la saisie.

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
