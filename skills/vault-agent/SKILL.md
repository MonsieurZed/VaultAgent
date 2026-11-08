---
name: vault-agent
description: Remplir et gérer des identifiants, TOTP et fiches partagées avec VaultAgent sans renvoyer les secrets au modèle. Utiliser avec plusieurs coffres/comptes via le client Windows/Linux associé, ou pour le remplissage via le service Windows autonome.
license: MIT
---

# VaultAgent

Avec le serveur **3.0.0-preview.4 ou ultérieur**, commencer par `vaults()` et choisir le coffre
autorisé correspondant à la demande. Lire [references/multi-vault.md](references/multi-vault.md)
pour la sélection explicite et les fiches communes Admin/Agent. Ne jamais substituer un coffre
plus privilégié à celui demandé pour contourner un refus.

Avec le **client associé au serveur 3**, si le coffre est verrouillé, utiliser la demande
de déverrouillage décrite dans [references/unlock.md](references/unlock.md). Elle notifie
Zed et attend sa validation Oui/Non, ou utilise l'autorisation automatique déjà choisie par Zed.
L'agent ne clique jamais sur l'approbation et ne modifie pas cette politique.

Pour **enregistrer ou gérer des fiches**, utiliser le client associé au serveur Linux
**3.0.0-preview.2 ou ultérieur** : lire [references/write.md](references/write.md).
Cette méthode sait créer des connexions, notes sécurisées et dossiers, puis modifier les champs
autorisés avec une révision attendue. Le service Windows autonome 2.x n'a pas cette API.

Pour le **remplissage via le service Windows autonome**, suivre les instructions ci-dessous.
Il est installé dans `%ProgramFiles%\HomelabVaultAgent`.
Les champs disponibles sont `username`, `password` et `totp`. Le catalogue fournit les UUID,
titres et origines des fiches ; les valeurs sont remplies par un helper sans être renvoyées au modèle.

## Préparer la cible et l'accès

- Vérifier le site ou l'application demandés par l'utilisateur et le champ visé. Pour un site,
  comparer l'origine avec `origins` dans le catalogue. Une fiche trouvée par son nom ne suffit pas.
  Si plusieurs comptes conviennent ou si la cible diffère, demander le choix avant de remplir.
- Les commandes doivent s'exécuter sur le PC Windows où VaultAgent est installé, avec le même
  compte et la même session Windows interactive. Un conteneur ou une session distante n'y accède
  pas automatiquement. Ne pas exposer le broker sur le réseau pour contourner cette limite.
- Le script local [scripts/vault.mjs](scripts/vault.mjs) accepte seulement les commandes ci-dessous.
  Résoudre son chemin depuis ce dossier de skill. Le lancer avec le Node embarqué de VaultAgent :

```powershell
$node = Join-Path $env:ProgramFiles 'HomelabVaultAgent\runtime\node.exe'
& $node '<chemin-du-skill>\scripts\vault.mjs' status
& $node '<chemin-du-skill>\scripts\vault.mjs' unlock
& $node '<chemin-du-skill>\scripts\vault.mjs' catalog 'nom du site'
```

`status` ne montre pas l'email du compte et ne prolonge pas la session. Si `configured=false`,
laisser l'utilisateur configurer son compte dans le bouclier violet ; ne pas lui demander son mot
de passe dans le chat. `unlock` déclenche la validation Windows configurée, UAC ou Hello.
Déverrouiller **avant** de donner le focus au champ. Ne pas sonder périodiquement le coffre pour
empêcher son expiration. Ne pas modifier le timeout ou le mode de protection pour terminer une connexion.

## Choisir le mode de remplissage

**Navigateur avec locator dans un runtime JavaScript local :** lire
[references/browser.md](references/browser.md). Utiliser `fillBrowser` avec le locator exact, l'UUID
choisi et le champ. Cela convient notamment au navigateur intégré lorsque ses outils exposent ce runtime.

**Application Windows ou navigateur piloté par le bureau :** après déverrouillage, donner le focus
au champ puis lancer sans ouvrir une nouvelle fenêtre console :

```powershell
& $node '<chemin-du-skill>\scripts\vault.mjs' type UUID-DE-LA-FICHE password
```

Le script utilise `windowsHide`; `type` utilise `SendInput` Unicode. Il vérifie la stabilité de la
fenêtre active, sans vérifier l'identité du champ ou du site : cette vérification appartient à l'agent.
La commande insère du texte dans la sélection courante. Si le champ contient déjà une valeur,
la sélectionner dans l'interface avant de remplir, sans la lire. Pour TOTP, utiliser `totp` : le service
calcule le code à la demande, sans exposer la graine. Traiter chaque champ séparément.

Si les outils ne peuvent ni exécuter les commandes localement ni utiliser un locator local,
expliquer l'outil manquant. Ne pas passer un secret comme paramètre de texte fabriqué par le modèle.
La disponibilité dans Claude Code ou Codex dépend de leurs outils réels ; ce skill n'installe aucun MCP.

## Vérifier et terminer

Le retour `{filled:true,...}` ou `{typed:true,...}` confirme l'opération de saisie, pas la connexion.
Soumettre le formulaire si la demande de l'utilisateur l'inclut, puis vérifier un résultat non sensible
comme la navigation ou un message de connexion. Éviter les captures et lectures DOM/AX après remplissage
qui révéleraient l'identifiant, un champ texte ou un secret rendu visible. Choisir le bouton de soumission
avant le remplissage si cela permet d'éviter une nouvelle inspection des champs.

Si la fenêtre change, redonner le focus et vérifier la cible avant une nouvelle saisie. En cas de
refus d'identifiants ou de code, arrêter les tentatives automatiques ; ne pas tester d'autres fiches
ou mots de passe. Un nouveau code TOTP peut être rempli si le site indique explicitement son expiration.
En cas d'annulation UAC/Hello, respecter l'annulation. Le verrouillage explicite est disponible :

```powershell
& $node '<chemin-du-skill>\scripts\vault.mjs' lock
```

Ne pas fermer de force une session partagée à la fin d'un remplissage ; conserver le délai choisi
par l'utilisateur, sauf demande de verrouillage ou fin d'une session temporaire convenue.

## Limites à respecter

- Ne pas lancer `get item`, `get password`, `get totp`, `list items`, `seal`, `worker-secret`, un export
  ou un autre profil CLI pour obtenir les valeurs. Ne pas lire les profils privés de ProgramData,
  `BW_SESSION`, les variables de déchiffrement du helper ou le presse-papiers.
- Conserver le helper installé : ne pas l'instrumenter pour afficher les données ni envoyer les
  valeurs à un outil ou service qui les ferait remonter dans la conversation.
- Cette voie évite le retour des secrets au modèle dans le flux normal. Elle ne protège pas contre
  du code arbitraire, un administrateur malveillant ou la page cible qui reçoit le secret.
- Le fournisseur Windows des passkeys du coffre et le parcours SSO interactif intégré ne sont pas
  livrés. Ne pas convertir une passkey en mot de passe ni présenter Hello comme un fournisseur de passkeys.
- Si le service est arrêté ou absent, diagnostiquer avec `Get-Service HomelabVaultAgent` et `status`.
  Ne pas réinstaller, oublier le compte ou désactiver TLS comme simple tentative de connexion.
