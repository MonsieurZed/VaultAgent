# VaultAgent

Service Windows pour Bitwarden et Vaultwarden : bouclier violet, moteur CLI persistant,
déverrouillage UAC ou Windows Hello et remplissage sans presse-papiers pour les agents.

## Installer

**[Télécharger l'installateur Windows x64](https://github.com/MonsieurZed/VaultAgent/releases/latest)**

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

## Passkeys Windows : developpement 2.1

Cette branche contient un fournisseur Windows experimental, avec creation ES256 dans le coffre,
signature locale et choix du compte dans une fenetre native. Chaque operation exige Windows Hello.
La release publique 2.0.1 et le skill installe ne contiennent pas cette fonction.

La compilation, les tests CTAP/signatures et les conversions des modeles du CLI sont verifies avec
des donnees factices. Le parcours complet navigateur → Windows → Hello → Vaultwarden reste a valider :
Hello est indisponible sur le poste de test dans la session RDP actuelle. Aucun support valide des
passkeys en RDP n'est annonce. Voir [construction, activation et limites](docs/passkeys.md).

## Interface, langues et canaux (2.1.1)

Les parametres distinguent **Compte**, **Securite** et **Application**. Chaque champ explique
son usage ; le code 2FA et le certificat sont facultatifs, et les options avancees sont repliees.
Le menu du bouclier **Langue et mises a jour** propose **Francais, English et Espanol**.
Le choix s'applique aux nouvelles fenetres ; certains diagnostics techniques du service ou de Windows
conservent leur langue d'origine. Les preferences sont propres au compte Windows, sans acces au coffre.

**Stable** consulte les releases GitHub publiques validees. **Experimental** inclut les preversions.
La recherche est manuelle ; aucun telechargement ni redemarrage silencieux. Le bouton d'installation
apparait uniquement pour une version plus recente. Le fichier vient du depot officiel, son SHA256
est compare au fichier de la meme release, puis l'installateur demande sa validation Windows.
Ce controle d'integrite n'est pas une signature d'editeur. Changer de canal ne force pas de retour
a une ancienne version. Un push de sources ne cree pas de release pour ces canaux.

## Désinstaller

Paramètres Windows → Applications → **VaultAgent** → Désinstaller. Le service, les exécutables et les
raccourcis sont retirés. Les comptes protégés et profils chiffrés restent dans ProgramData pour réinstallation.
Le bouton **Oublier le compte** supprime séparément la configuration protégée après UAC.

## Skill pour Codex et Claude Code

Le [skill vault-agent](skills/vault-agent/SKILL.md) guide le choix d'une fiche et le remplissage
username/password/TOTP sans faire remonter les valeurs au modèle. Il inclut un petit helper local
dont les commandes sont limitées à status, catalogue, validation Windows, verrouillage et saisie.
Le [ZIP du skill](https://github.com/MonsieurZed/VaultAgent/releases/download/v2.0.1/vault-agent-skill.zip)
est également disponible pour copier son dossier `vault-agent` dans le répertoire de skills de l'agent.

Depuis un clone du dépôt, sans élévation :

```powershell
./install-agent-skill.ps1 -Agent Both
```

Codex : `$vault-agent`, installé dans `~/.agents/skills/vault-agent`.
Claude Code : `/vault-agent`, installé dans `~/.claude/skills/vault-agent`.
Un seul agent peut être sélectionné avec `-Agent Codex` ou `-Agent Claude`.
Pour retirer les fichiers installés inchangés : `./install-agent-skill.ps1 -Agent Both -Uninstall`.
Le script refuse d'écraser un skill existant. Si la découverte ne se rafraîchit pas, rouvrir la session.

Ce skill suppose un outil capable d'exécuter les commandes sur le PC Windows, dans la session de
l'utilisateur. Il n'installe pas de MCP ni d'accès distant. Une session Claude/Codex dans le cloud
ne gagne pas l'accès au service Windows en installant simplement ces instructions.

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
