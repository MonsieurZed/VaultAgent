//! Textes de la mise en page : barre d'état, ligne du coffre, demandes à valider et messages.

/// Entrées (français, anglais, espagnol).
pub const ENTRIES: &[(&str, &str, &str)] = &[
    (
        "Ce compte est déjà configuré dans un autre coffre.",
        "This account is already configured in another vault.",
        "Esta cuenta ya está configurada en otra bóveda.",
    ),
    ("À valider", "To approve", "Por aprobar"),
    ("Demande de déverrouillage", "Unlock request", "Solicitud de desbloqueo"),
    ("Refuser", "Deny", "Rechazar"),
    ("Déverrouiller", "Unlock", "Desbloquear"),
    ("Verrouiller", "Lock", "Bloquear"),
    ("Verrouillé", "Locked", "Bloqueada"),
    ("Déverrouillé", "Unlocked", "Desbloqueada"),
    ("Déverrouillé jusqu’à", "Unlocked until", "Desbloqueada hasta"),
    ("À configurer", "Setup required", "Configuración pendiente"),
    ("Aide", "Help", "Ayuda"),
    ("Modifier", "Edit", "Modificar"),
    ("sur", "on", "en"),
    ("verrouillage après", "locks after", "bloqueo tras"),
    ("min", "min", "min"),
    (
        "sans verrouillage automatique",
        "no automatic lock",
        "sin bloqueo automático",
    ),
    ("Opération en cours…", "Working…", "Operación en curso…"),
    ("Enregistré.", "Saved.", "Guardado."),
    (
        "Connexion refusée. Vérifiez le mot de passe maître du coffre et le code 2FA éventuel.",
        "Sign-in refused. Check the vault master password and 2FA code if required.",
        "Inicio de sesión rechazado. Comprueba la contraseña maestra de la bóveda y el código 2FA si se requiere.",
    ),
    (
        "Authentification expirée. Rechargez la page pour vous reconnecter.",
        "Authentication expired. Reload to sign in again.",
        "Autenticación caducada. Recarga la página para volver a entrar.",
    ),
    ("Actualiser", "Refresh", "Actualizar"),
    (
        "Cette demande a expiré.",
        "This request has expired.",
        "Esta solicitud ha caducado.",
    ),
    ("Langue", "Language", "Idioma"),
    ("Coffre", "Vault", "Bóveda"),
    ("Rien à valider", "Nothing to approve", "Nada que aprobar"),
    (
        "La page se met à jour seule.",
        "The page updates by itself.",
        "La página se actualiza sola.",
    ),
];
