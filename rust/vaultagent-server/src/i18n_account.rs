//! Textes de l’onglet « Compte » : formulaire de connexion au coffre.

/// Entrées (français, anglais, espagnol).
pub const ENTRIES: &[(&str, &str, &str)] = &[
    ("Connecter votre coffre", "Connect your vault", "Conectar tu bóveda"),
    (
        "À configurer une fois. Le mot de passe n’est jamais renvoyé par l’API.",
        "Set up once. The API never returns the password.",
        "Configura una vez. La API nunca devuelve la contraseña.",
    ),
    (
        "Adresse HTTPS du coffre",
        "HTTPS vault address",
        "Dirección HTTPS de la bóveda",
    ),
    (
        "L’adresse que vous ouvrez pour accéder à Bitwarden ou Vaultwarden.",
        "The address you open to access Bitwarden or Vaultwarden.",
        "La dirección que abres para acceder a Bitwarden o Vaultwarden.",
    ),
    ("Adresse email", "Email address", "Correo electrónico"),
    (
        "Le compte de votre coffre, pas nécessairement celui du SSO.",
        "Your vault account, which may differ from your SSO account.",
        "La cuenta de tu bóveda, que puede ser distinta de tu cuenta SSO.",
    ),
    ("Mot de passe maître", "Master password", "Contraseña maestra"),
    (
        "Déjà enregistré ? Laissez vide pour le conserver. Ne le transmettez pas dans le chat.",
        "Already saved? Leave blank to keep it. Never send it in chat.",
        "¿Ya está guardada? Déjalo vacío para conservarla. Nunca la envíes por el chat.",
    ),
    ("Code 2FA · facultatif", "2FA code · optional", "Código 2FA · opcional"),
    (
        "Seulement si la double authentification est activée sur ce coffre.",
        "Only if two-factor authentication is enabled for this vault.",
        "Solo si la autenticación de dos factores está activada en esta bóveda.",
    ),
    ("Après utilisation", "After use", "Después del uso"),
    (
        "Verrouiller automatiquement",
        "Lock automatically",
        "Bloquear automáticamente",
    ),
    ("Délai en minutes", "Timeout in minutes", "Tiempo en minutos"),
    (
        "Chaque utilisation autorisée prolonge le délai. Le redémarrage du serveur verrouille toujours le coffre.",
        "Each authorized use extends the timeout. Restarting the server always locks the vault.",
        "Cada uso autorizado prolonga el plazo. Reiniciar el servidor siempre bloquea la bóveda.",
    ),
    ("Vérifier et enregistrer", "Verify and save", "Verificar y guardar"),
    (
        "Changer de compte révoque les clients associés. Windows Hello et les passkeys Windows ne sont pas fournis par ce serveur.",
        "Changing accounts revokes paired clients. This server does not provide Windows Hello or Windows passkeys.",
        "Cambiar de cuenta revoca los clientes asociados. Este servidor no proporciona Windows Hello ni passkeys de Windows.",
    ),
];
