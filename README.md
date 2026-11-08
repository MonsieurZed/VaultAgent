# VaultAgent

Service Windows pour Bitwarden et Vaultwarden : bouclier violet, moteur CLI persistant,
déverrouillage UAC ou Windows Hello et remplissage sans presse-papiers pour les agents.

## Installer

Lancer `VaultAgent-Setup-2.0.1-x64.exe`, puis **Installer** et valider l'UAC avec le même compte Windows.
L'EXE contient l'application, Node et le runtime .NET : aucun SDK ni PowerShell 7 à installer.
Il embarque .NET Desktop 10.0.14 et propose les raccourcis bureau/démarrage et une entrée **VaultAgent**
dans les applications installées Windows.

Après installation, **Configurer le compte** ouvre les paramètres : serveur HTTPS, email, mot de passe
maître et code 2FA éventuel. Le compte existant est conservé lors d'une mise à jour. Le coffre est verrouillé
au redémarrage du service et demande une nouvelle validation Windows.

L'installateur ne contient aucun compte, profil, mot de passe maître ou certificat privé utilisateur.
L'EXE n'est actuellement pas signé par un certificat d'éditeur ; Windows peut afficher « éditeur inconnu ».
Le fichier `.sha256` permet de vérifier l'intégrité du téléchargement, sans constituer une signature d'éditeur.

## Utiliser

Le bouclier près de l'horloge permet de déverrouiller, verrouiller et ouvrir les paramètres.
Le délai glissant est réglable de 1 à 1440 minutes, 15 par défaut, ou désactivable. Verrouillage Windows,
déconnexion et verrouillage manuel ferment toujours la session. `status` ne prolonge pas sa durée.
Windows Hello dépend du PIN/visage/empreinte disponible sur le poste ; UAC reste un mode distinct.

```powershell
$vault = 'C:\Program Files\HomelabVaultAgent\VaultAgent.exe'
& $vault status
& $vault unlock
& $vault catalog 'nom du site'
& $vault lock
```

Le catalogue renvoie seulement les métadonnées nécessaires pour choisir une fiche. En mode agents sans
lecture, les commandes CLI de lecture des secrets sont refusées. Le broker ne publie aucun port TCP.

```javascript
var vault = await import('file:///C:/Program%20Files/HomelabVaultAgent/agent-tools.mjs');
nodeRepl.write(await vault.fillBrowser({
  locator: tab.playwright.getByLabel('Mot de passe', { exact: true }),
  itemId: 'UUID de fiche obtenu dans le catalogue', field: 'password'
}));
```

Champs disponibles : `username`, `password`, `totp`. Le helper déchiffre localement et remplit le locator ;
seule une confirmation est renvoyée au modèle. Aucune extension de navigateur ou presse-papiers.
Pour Windows, donner le focus au champ après déverrouillage puis appeler
`vault.typeWindows({itemId:'UUID',field:'totp'})`. La saisie Unicode vérifie la fenêtre active.

Ce flux évite de montrer les valeurs au modèle dans l'usage normal. Il ne constitue pas une isolation
contre du code arbitraire, un administrateur malveillant ou la page qui reçoit le secret.
Ne jamais afficher les variables privées du helper, lire le DOM rempli ou journaliser des secrets.

Le SSO du fournisseur et le déverrouillage du coffre sont distincts. Le formulaire actuel utilise
email/mot de passe maître ; aucun parcours SSO interactif intégré n'est livré.
L'authentification Hello du service est présente ; un fournisseur Windows des passkeys du coffre ne l'est pas.

## Désinstaller

Paramètres Windows → Applications → **VaultAgent** → Désinstaller. Le service, les exécutables et les
raccourcis sont retirés. Les comptes protégés et profils chiffrés restent dans ProgramData pour réinstallation.
Le bouton **Oublier le compte** supprime séparément la configuration protégée après UAC.

## Construire

Windows x64, SDK .NET 9, Node 22, npm, git et PowerShell 7 pour les scripts de construction :

```powershell
./build-engine.ps1
./build-installer.ps1
```

`build-engine.ps1` récupère la révision Bitwarden épinglée si nécessaire. Sortie : `dist`.
L'installateur est compilé avec le SDK Microsoft ; aucun compilateur tiers n'est nécessaire.
`./prepare-package.ps1` et `./install.ps1` restent disponibles pour une installation de développement.

## Vérification

Tests .NET : `dotnet publish` exécute les vérifications via `prepare-package.ps1` : DPAPI, durée,
signatures Hello, transfert chiffré et bornes IPC. `--engine-smoke` vérifie 20 commandes sur profil isolé.
`tests/login-parity.cjs` compare les preuves envoyées avec le CLI officiel 2026.10.1 sur un serveur HTTPS
éphémère, uniquement avec identifiants factices. Le certificat reste limité aux processus de test.
Le test suppose le CLI officiel dans `%LOCALAPPDATA%\Homelab\VaultCli\node_modules\@bitwarden\cli`.

Le défaut initial de dérivation après changement de serveur a été reproduit et corrigé : le serveur est
préparé dans le profil et le moteur redémarré avant réception du mot de passe. Une connexion réelle a
ensuite été confirmée. Dernier contrôle `status` complet avec lancement de l'EXE : ~325 ms, 3 essais.
Les mesures dans `benchmark.json` concernent un profil vide et ne décrivent pas tous les appels réseau.

Licences et code du moteur : [THIRD-PARTY.md](THIRD-PARTY.md).
