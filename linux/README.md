# Serveur Linux et clients — 3.0.0-preview.3

## Demandes de déverrouillage

Un client associé peut appeler `requestUnlock()` ou `unlock({waitSeconds:60})` du helper.
Dans Clients, le propriétaire peut cocher « Toujours accepter les demandes de ce client ».
Ce réglage persiste sur Linux : le client autorisé demande l'ouverture sans dépendre de Windows.
Le délai reste actif ; la prochaine demande autorisée rouvre le coffre après expiration.
Décocher cette option retire cette autorisation. Une erreur 2FA n'est jamais contournée.

Pour une confirmation manuelle, le PC Windows approbateur montre une petite notification
Oui/Non, avec une case Toujours accepter. Aucun navigateur ni UAC à chaque demande.
L'accord ouvre le coffre pour les clients associés selon le délai existant. Le menu du bouclier
permet de désactiver toutes les acceptations automatiques. Le portail Authentik reste une voie de secours.

Installer un PC approbateur : `VaultAgent-Client.exe approval-public-key` retourne seulement
son UUID et sa clé publique. Côté administration du serveur arrêté, transmettre ce JSON à
`node linux/register-approver.mjs` avec les données montées sous `/data`, puis redémarrer.
Ne pas exposer cette commande administrative comme route de client. Les décisions sont signées
RSA-PSS, liées à une demande aléatoire et expirante ; le client ordinaire ne peut pas approuver.
La clé privée est protégée par DPAPI utilisateur Windows. Cela protège le flux normal, pas
un programme malveillant exécuté avec les droits de ce même utilisateur ou de l'administrateur.

Une demande dure cinq minutes, est dédupliquée par appareil et n'allonge pas la session.
Un refus est consommé jusqu'à expiration. Une révocation, un verrouillage ou un changement
de compte invalide les anciennes demandes. Les demandes restent en mémoire et disparaissent
au redémarrage. Le helper attend au plus 300 secondes, puis renvoie un état sans relancer.

La preview.2 ajoute l'écriture dédiée sans lecture pour les clients associés : création de
connexions, notes sécurisées et dossiers, prévisualisation des métadonnées et édition ciblée.
Voir [le protocole](../skills/vault-agent/references/write.md). Le serveur vérifie chaque écriture
par relecture interne et conserve des reçus idempotents dans son volume privé. Une interruption
après intention d'écriture bloque les répétitions automatiques plutôt que risquer un doublon.
Les reçus ne contiennent aucun secret ; leur empreinte interne est un HMAC avec la clé machine.
La limite actuelle est 10 000 opérations conservées ; aucune purge automatique ne compromet
la protection contre les répétitions. Les opérations d'autres clients externes restent soumises
aux contrôles de révision du SDK/serveur ; le verrou de sérialisation ne couvre que VaultAgent.

Serveur deploye sur Maison : https://vaultagent.lan.zed.cx. Authentik protege
l'administration et l'association des clients utilise un code unique valable cinq minutes.
La session du moteur est persistante, avec delai glissant configurable ou desactivable.
Tout redemarrage reverrouille le coffre. Les clients ne peuvent ni ouvrir ni configurer le coffre.

Le client Windows est un ZIP portable, sans service ni installation administrateur.
Lancer VaultAgent-Client.exe et saisir l'URL du serveur puis le code cree dans l'onglet Clients.
Le client Linux est un TAR avec Node 22 inclus ; voir remote-client/README.md.
Les deux paquets sont telechargeables dans l'onglet Clients, apres connexion Authentik.

## Construction

- `build-remote-client.ps1` compile uniquement le client Windows et son runtime .NET.
- `prepare-linux-stack.ps1` copie les sources publiques dans le contexte Docker de Reseau.
- `linux/Dockerfile` construit le moteur GPL depuis la revision Bitwarden epinglee et les lockfiles.
- Node 22.23.4 et son image sont epingles par digest. Aucun profil prive n'est copie.
- Le deploiement reel est dans Reseau/stacks/maison/vaultagent ; le proxy SSO est sur OPNsense/Web.

Le serveur attend VAULTAGENT_ORIGIN (origine HTTPS exacte), VAULTAGENT_OWNER (nom Authentik)
et VAULTAGENT_TRUSTED_PROXY (IP exacte du proxy interne). Son port 8080 n'est jamais publie.
Les requetes d'administration exigent l'identite transmise par le proxy Authentik, et les
mutations exigent aussi une origine identique. Le proxy public efface les en-tetes d'identite
fournis par le visiteur. Les routes agents utilisent uniquement le jeton propre au client.

## Donnees et limites

Le mot de passe maitre est chiffre par AES-GCM dans le volume prive. La cle serveur est dans
le meme volume, fichiers 0600 et dossier 0700. Cela ne protege pas d'un administrateur du serveur
ou d'une compromission du processus. Sauvegarder ce volume uniquement sous forme chiffree.
Les jetons clients sont stockes sous forme de hachages cote serveur. Windows protege le jeton
par DPAPI utilisateur ; Linux le conserve dans un fichier prive 0600.

Les enveloppes utilisent RSA-OAEP/SHA256 et AES-256-GCM. Seule une confirmation revient au modele
apres le remplissage. L'origine HTTPS doit correspondre exactement a une URL de la fiche.
Un agent avec du code arbitraire ou le controle du DOM peut encore recuperer un champ rempli :
ce flux evite l'affichage normal, sans fournir une isolation contre cet agent.
Les passkeys du fournisseur Windows experimental ne sont pas portees dans cette preview.

## Validation effectuee

Neuf tests avec donnees factices passent sur Windows et Linux : chiffrement au repos,
expiration/prolongation, enveloppes, origines, association, expiration des codes, revocation,
changement de compte, refus des routes brutes et protections des routes admin.
Le helper HTTPS remplit un locator factice sans retourner sa valeur ; l'interoperabilite
Node/.NET est verifiee independamment. Le moteur Linux demarre avec un profil isole non connecte.
Le proxy redirige les visiteurs anonymes vers Authentik et le backend direct renvoie 403.

Le client Windows a ete installe et son association au serveur Linux a ete verifiee par un appel
authentifie de statut. Le demarrage automatique et le processus en arriere-plan sont verifies.
L'installateur conserve l'association existante ; la fenetre propose des exemples, sans adresse
preremplie. Le coffre peut rester verrouille jusqu'a une autorisation explicite dans l'interface web.
