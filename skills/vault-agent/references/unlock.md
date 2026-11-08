# Demander le déverrouillage — serveur/client 3.0.0-preview.3 ou ultérieur

Depuis preview.5, Zed peut activer Windows Hello depuis le bouclier du client distant.
Un accord manuel sur un coffre activé exige alors une preuve Hello vérifiée par Linux.
L'agent demande et attend ; il ne configure pas Hello, n'enrôle pas de clé et ne manipule
jamais le PIN ou la boîte de dialogue Windows. Respecter annulation et indisponibilité.
`hello-status` vérifie seulement la disponibilité. `hello-setup` est réservé à une demande
explicite de configuration par l'utilisateur. Les clients déjà autorisés automatiquement
restent automatiques. La présence du menu ne prouve pas que Hello est activé sur le PC.

Importer le helper du client installé, sans lire son fichier d'association :

```javascript
var remote = await import('file:///C:/Users/Zed/AppData/Local/Programs/VaultAgentClient/remote-agent-tools.mjs');
nodeRepl.write(await remote.requestUnlock());
```

Résoudre le profil Windows courant plutôt que recopier le nom d'utilisateur de cet exemple.
Sous Linux, importer `remote-client/agent-tools.mjs` du paquet installé.
`requestUnlock()` renvoie l'UUID publique et l'état de la demande, sans ouvrir le coffre.
Le PC Windows approbateur affiche une petite notification Oui/Non et une case Toujours accepter.
La politique Toujours accepter est conservée par client sur Linux ; si Zed l'a activée, le serveur
ouvre le coffre à la demande sans dépendre de Windows. L'agent ne doit ni activer cette politique,
ni enregistrer un nouveau PC approbateur de sa propre initiative. Zed peut la désactiver dans
Clients ou par le menu du bouclier. Avant preview.5, la confirmation distante ne fournit pas Hello.

L'agent **ne doit pas approuver sa propre demande**, appeler les routes administrateur,
cliquer sur Oui/Autoriser les clients/Autoriser cette demande ni piloter la session web de Zed
pour obtenir son accord. Laisser cette action à l'utilisateur, même si le navigateur est ouvert.

Pour attendre l'accord dans le même runtime :

```javascript
nodeRepl.write(await remote.unlock({ waitSeconds: 60 }));
```

L'attente est bornée à 300 secondes ; limiter chaque appel aux contraintes de durée de l'outil.
Un appel sans attente renvoie immédiatement. Reprendre seulement si `unlocked:true` ;
`pending`/`waiting` demandent encore l'accord. Après `denied`, `failed` ou `expired`, arrêter
et informer Zed sans relancer automatiquement. Les demandes expirent après cinq minutes,
restent dédupliquées et ne prolongent pas la session du coffre. Un verrouillage, une révocation
ou un changement de compte invalide une ancienne demande.

CLI Linux : `vaultagent-client unlock` ou `vaultagent-client unlock 60`.
CLI Windows : `VaultAgent-Client.exe request-unlock` ; utiliser le helper pour l'attente.
L'approbation rend le coffre disponible aux clients associés pendant le délai déjà configuré.
Elle ne réalise pas l'opération métier demandée : après l'accord, relancer cette opération.
