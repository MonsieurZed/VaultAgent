# Remplir un navigateur avec un locator local

Utiliser cette voie seulement dans le runtime de l'outil qui possède le locator. Le module emploie
Node et le client VaultAgent Windows ; il ne s'exécute pas dans `page.evaluate` ni dans la console de la page.
Aucun plugin de navigateur et aucun presse-papiers ne sont nécessaires.

Dans un runtime Node local avec accès au locator :

```javascript
var vault = await import('file:///C:/Users/Zed/AppData/Local/Programs/VaultAgentClient/remote-agent-tools.mjs');
var result = await vault.fillBrowser({
  locator: tab.playwright.getByLabel('Mot de passe', { exact: true }),
  itemId: 'UUID-DE-LA-FICHE',
  field: 'password',
  origin: 'https://adresse-exacte-de-la-page'
});
nodeRepl.write(result);
```

`tab.playwright` et `nodeRepl.write` illustrent l'environnement du navigateur intégré Codex.
Dans un outil Playwright Node classique, fournir par exemple `page.getByLabel(...)` et renvoyer
seulement `result` via la sortie de l'outil. Adapter aux APIs effectivement documentées par l'outil.
Claude Code peut utiliser cette voie si ses outils exposent ce runtime local et ce locator ;
un simple outil de navigateur attendant un texte en argument ne fournit pas cette capacité.

Résoudre le profil Windows courant plutôt que recopier le nom d'utilisateur de l'exemple. L'origine HTTPS doit
correspondre exactement à une adresse de la fiche (`origins` du catalogue). Déverrouiller au préalable avec le script `vault.mjs`, puis sélectionner un locator unique dans la
bonne page et le bon frame. Un `fillBrowser` réussi remplace la valeur du champ et renvoie seulement
`{filled:true,field:'password',clipboardUsed:false}`. Utiliser `username` pour l'identifiant et `totp`
pour un code authentificateur. Ne pas utiliser le mode `probe` pour une connexion réelle.

Le helper génère une paire de clés éphémère, reçoit une enveloppe chiffrée, déchiffre localement et
appelle `locator.fill` dans sa fermeture. Ne pas recopier cette logique dans la conversation ni ajouter
un affichage des variables, une lecture DOM, `inputValue`, une trace Playwright avec les arguments,
un enregistrement de requêtes ou une capture qui montrerait les valeurs.

S'il n'existe aucun runtime adapté, utiliser la saisie Windows avec une fenêtre visible et un champ
correctement focalisé, en fournissant `--process` et `--title` issus de `foreground`. Si le navigateur est virtuel et ne reçoit pas `SendInput`, signaler cette limite ;
ne pas supposer que le clavier système contrôle un onglet virtuel.
