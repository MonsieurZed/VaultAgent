# Travail sur VaultAgent

- Lire README.md et THIRD-PARTY.md ; conserver les licences et la révision upstream épinglée.
- Ne jamais lire des comptes ou profils privés ni demander un mot de passe dans le chat.
- Ne jamais exporter de session Bitwarden, capturer un secret ou journaliser les requêtes de connexion.
- Tester avec profils isolés et identifiants factices ; conserver la validation TLS.
- Le helper renvoie seulement une confirmation. Ne pas lire ses variables privées ou le DOM après remplissage.
- Le fournisseur OS de passkeys de la branche 2.1 est experimental, sans validation complete Hello/navigateur a ce jour. La release 2.0.1 ne le contient pas. Aucun parcours SSO interactif integre n'est livre.
- Les passkeys exigent une preuve Windows Hello fraiche par operation ; un consentement UAC seul ne remplace pas la verification utilisateur WebAuthn.
- Les fichiers générés, dépendances et états locaux restent exclus du dépôt.
- Commentaires et documentation en français ; fichiers ciblés, sous 300 lignes autant que possible.
- Pas de commit, push, tag ni publication de release sans autorisation explicite de Zed.
