# Windows Hello pour le serveur Linux — preview.5

Depuis le bouclier Windows, choisir **Activer Windows Hello** et valider dans Windows.
Le client doit deja etre un PC approbateur enregistre sur les coffres concernes.
`VaultAgent-Client.exe hello-status` indique la disponibilite sans lancer de verification ;
`hello-setup` ouvre uniquement l'ecran de configuration, sans saisie de PIN dans la CLI.

L'enrolement utilise WebAuthn natif avec authentificateur de plateforme et verification
utilisateur obligatoire. Le serveur emet un nonce d'enrolement ; la cle d'approbation deja
enregistree signe la transaction et la nouvelle cle Hello prouve sa possession. Linux conserve
seulement l'identifiant et la cle publique. Le PIN et les donnees biometriques restent dans Windows.
Les coffres actives sont verrouilles pour invalider les anciennes demandes.

Sur un coffre active, chaque PC qui approuve manuellement doit avoir une cle Hello enregistree.
Apres Oui, le client demande la preuve Hello : le serveur verifie type, origine, RP ID,
challenge derive de la transaction, presence, verification utilisateur et signature EC2/RSA.
Le challenge lie serveur, coffre, demande, decision et option Toujours accepter. Une signature
RSA d'approbation seule ne suffit plus. Le client ne peut pas remplacer une cle Hello existante.
L'ajout d'un autre PC apres activation exige une autorisation d'enrolement par l'administration
locale (`register-approver.mjs`) ; son ancien acces RSA seul ne permet pas d'enregistrer une cle.

Non reste disponible sans Hello. Une annulation ou une indisponibilite n'entraine aucune
validation de secours. Les acceptations automatiques explicitement choisies restent valables,
et le proprietaire Authentik conserve son parcours d'administration dans le portail.
Une activation interrompue peut avoir protege certains coffres ; une nouvelle activation
reprend les coffres restants. Une panne apres enregistrement de la politique echoue en fermeture,
avec reprise de l'enrolement ou recours au portail du proprietaire.

Si Windows annonce Hello indisponible, configurer Windows Hello depuis une session locale
appropriee. Le RDP peut empecher son utilisation ; ne pas contourner le refus ni demander le
PIN dans le chat. L'integration physique Hello ne peut pas etre validee avec des preuves factices.
Cette fonctionnalite ne cree pas de passkeys pour les sites et ne modifie pas le fournisseur OS experimental.

Les assertions suivent [WebAuthn](https://www.w3.org/TR/webauthn-3/) et l'API
[Windows GetAssertion](https://learn.microsoft.com/en-us/windows/win32/api/webauthn/nf-webauthn-webauthnauthenticatorgetassertion).
Le controle ne certifie pas a distance le TPM ou le materiel : aucune chaine d'attestation
materielle n'est validee. Le poste et son enrolement initial doivent etre de confiance.
Une ancienne version du serveur ne sait pas imposer Hello ; ne pas la substituer a un coffre
active sans tenir compte de cette perte de controle. Les sauvegardes doivent conserver la
politique approval-policy.json et les cles publiques des appareils avec le reste des donnees.
