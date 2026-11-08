# Fournisseur Windows de passkeys — experimental

Etat au 8 novembre 2026 : application experimentale 2.1.0 installee, service demarre et compte
conserve. Revision 2.1.1 : interface simplifiee, langues et choix du canal de mise a jour.
Le parcours complet des passkeys reste non valide. Aucun fournisseur ou certificat de confiance
n'a ete enregistre sur le poste de developpement. La release stable publique reste 2.0.1.

## Conditions et activation

- Windows x64 recent avec les exports WebAuthNPluginAddAuthenticator2 et
  WebAuthNPluginPerformUserVerification2 ; le programme verifie leur presence.
- Windows Hello configure pour ce compte. Dans la session RDP de test, Hello est indisponible.
  Configurer le PIN et faire la premiere validation directement devant le PC.
- Compte Bitwarden/Vaultwarden deja configure dans VaultAgent.

Construire puis lancer l'installateur experimental. Dans le menu du bouclier violet,
ouvrir **Passkeys Windows**. Installer l'identite du fournisseur pour l'utilisateur courant,
enregistrer VaultAgent, puis l'activer dans les options avancees des passkeys Windows.
Le bouton d'enregistrement reste desactive si Hello est indisponible.

Le package MSIX contient une identite COM avec emplacement externe dans Program Files.
Seul le lanceur VaultAgent.Provider.exe porte cette identite ; le service SYSTEM conserve
son executable Windows classique VaultAgent.exe. Avec un
certificat autosigne de developpement, une confirmation montre son empreinte SHA256 avant tout
ajout dans **Utilisateur courant / Personnes de confiance**. Aucun ajout dans les autorites racines,
aucun changement des strategies Windows ou du TPM. Le certificat public seul est distribue.
Ne pas faire confiance a un certificat d'une provenance inconnue.

La desinstallation retire l'identite de l'utilisateur qui la lance et le certificat uniquement
si notre script l'avait ajoute. Les inscriptions d'autres comptes Windows doivent etre retirees
depuis ces comptes avant une desinstallation commune. Les donnees du coffre sont conservees.

## Construction reproductible

SDK .NET 9, runtime Desktop 10.0.14 embarque, Node/npm et Windows SDK MakeAppx/SignTool.
Les sources Bitwarden restent epinglees a la revision indiquee dans THIRD-PARTY.md.

```powershell
./build-engine.ps1 -Source 'CHEMIN_DES_SOURCES_BITWARDEN'
./build-passkey-identity.ps1 -DevelopmentCertificate
./build-installer.ps1
node tests/passkey-models.cjs 'CHEMIN_DES_SOURCES_BITWARDEN'
node tests/login-parity.cjs
```

La construction ne fait pas confiance au certificat et n'enregistre pas le fournisseur. Pour une
distribution de production, utiliser un certificat d'editeur correspondant au Publisher du
manifeste avec `-CertificateThumbprint`, sans `-DevelopmentCertificate`.

## Traitement des operations

Windows active le serveur COM protege. Le service verifie le chemin du processus, son identite
de package et la signature Windows du CTAP avant de demander l'ouverture du coffre. Le service
cree ensuite un challenge aleatoire a usage unique, lie au processus, a la session et a une
expiration de deux minutes. La signature Hello de ce challenge est requise avant toute creation
ou assertion. Un verrouillage invalide les operations en attente.

La cle privee P-256 est creee et utilisee dans le service. Elle est enregistree en PKCS8/base64url
dans une nouvelle fiche du coffre, via le moteur Bitwarden et son chiffrement. Une relecture
doit confirmer les champs avant de retourner l'attestation publique. Le navigateur recoit les
donnees WebAuthn publiques et signatures ; les agents ne recoivent aucune cle privee.
Le cache Windows contient des metadonnees de compte, jamais les cles. **Actualiser les passkeys**
reconstruit ce cache apres ouverture du coffre.

## Limites connues et validation restante

- ES256 / P-256, attestation `none`, passkeys synchronisables et compteur zero uniquement.
  Les credentials importes avec compteur non nul sont refuses, sans mise a zero.
- PRF, hmac-secret, largeBlobKey et credBlob ne sont pas pris en charge.
- Le delai glissant du coffre ne dispense pas de Hello pour chaque operation WebAuthn.
  Le mode UAC continue de servir a ouvrir le coffre ; il ne constitue pas la verification Hello.
- Une interruption apres acceptation par le serveur peut laisser une fiche creee sans que le
  site ait termine son enregistrement. Aucun nouvel essai ni effacement automatique.
- Activation MSIX/COM, choix du fournisseur dans le navigateur, premiere creation, connexion,
  annulation, verrouillage concurrent et conservation apres redemarrage restent a tester en
  session locale. Le comportement RDP et celui du navigateur integre ne sont pas garantis.
- Ne pas utiliser ces passkeys experimentales comme unique moyen d'acces a un compte.

Tests deja passes : CBOR CTAP accepte par le decodeur Windows, authenticatorData/COSE et signature
ES256 verifiee independamment, refus d'un autre RP et de preuves alterees, tailles ABI x64,
conversion des champs par les classes reelles du CLI et signature avec la cle factice relue.
Le test login-parity confirme aussi que l'adaptation conserve la derivation de connexion officielle.

Sources d'interface : [Microsoft WebAuthn](https://github.com/microsoft/webauthn),
[PasskeyManager](https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/passkeymanager/),
[identite pour application non empaquetee](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-nonpackaged-apps).
