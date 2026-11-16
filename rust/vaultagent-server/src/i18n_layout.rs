//! Textes de la mise en page : en-tête, statut, onglets, « Fonctionnement », messages et demandes de déverrouillage.

/// Entrées (français, anglais, espagnol).
pub const ENTRIES: &[(&str, &str, &str)] = &[
    (
        "Ce compte est déjà configuré dans un autre coffre.",
        "This account is already configured in another vault.",
        "Esta cuenta ya está configurada en otra bóveda.",
    ),
    (
        "Demandes de déverrouillage",
        "Unlock requests",
        "Solicitudes de desbloqueo",
    ),
    (
        "Votre accord ouvre le coffre aux clients associés pour le délai configuré. Code 2FA ci-dessus si nécessaire.",
        "Approval opens the vault to paired clients for the configured timeout. Enter 2FA above if needed.",
        "La aprobación abre la bóveda a los clientes asociados durante el plazo configurado. Introduce 2FA arriba si hace falta.",
    ),
    (
        "Autoriser cette demande",
        "Approve this request",
        "Aceptar esta solicitud",
    ),
    ("Refuser", "Deny", "Rechazar"),
    (
        "Votre coffre, disponible sans votre PC",
        "Your vault, available without your PC",
        "Tu bóveda, disponible sin tu PC",
    ),
    (
        "Un coffre. Vos clients. Le contrôle reste à vous.",
        "One vault. Your clients. You stay in control.",
        "Una bóveda. Tus clientes. Tú mantienes el control.",
    ),
    (
        "Le serveur conserve le moteur du coffre. Les clients remplissent les champs sans afficher les secrets aux agents.",
        "The server runs the vault engine. Clients fill fields without displaying secrets to agents.",
        "El servidor mantiene el motor de la bóveda. Los clientes rellenan campos sin mostrar secretos a los agentes.",
    ),
    ("Autoriser les clients", "Authorize clients", "Autorizar clientes"),
    ("Verrouiller", "Lock", "Bloquear"),
    ("Compte", "Account", "Cuenta"),
    ("Clients", "Clients", "Clientes"),
    ("Fonctionnement", "How it works", "Funcionamiento"),
    (
        "Ce que protège le remplissage",
        "What blind filling protects",
        "Qué protege el rellenado",
    ),
    (
        "Les mots de passe et codes TOTP sont déchiffrés dans le helper du client et remplis sans presse-papiers. L’agent reçoit seulement une confirmation.",
        "Passwords and TOTP codes are decrypted inside the client helper and filled without the clipboard. The agent receives only a confirmation.",
        "Las contraseñas y códigos TOTP se descifran dentro del helper del cliente y se rellenan sin portapapeles. El agente solo recibe una confirmación.",
    ),
    (
        "Un agent qui contrôle entièrement le navigateur peut encore lire un champ rempli. Ce mécanisme évite l’affichage des secrets dans le flux normal ; il ne protège pas contre du code arbitraire malveillant.",
        "An agent with full browser control can still read filled fields. This avoids displaying secrets during normal use; it does not protect against arbitrary malicious code.",
        "Un agente con control total del navegador puede leer los campos rellenados. Esto evita mostrar secretos durante el uso normal; no protege contra código arbitrario malicioso.",
    ),
    (
        "Authentik protège l’administration. Chaque client possède une autorisation distincte et révocable. Aucun accès brut aux secrets n’est proposé.",
        "Authentik protects administration. Each client has a separate, revocable authorization. No raw-secret endpoint is provided.",
        "Authentik protege la administración. Cada cliente tiene una autorización independiente y revocable. No se ofrece acceso a los secretos sin cifrar.",
    ),
    (
        "Accès privé LAN / VPN",
        "Private LAN / VPN access",
        "Acceso privado LAN / VPN",
    ),
    ("Coffre verrouillé", "Vault locked", "Bóveda bloqueada"),
    (
        "Coffre disponible pour les clients",
        "Vault available to clients",
        "Bóveda disponible para los clientes",
    ),
    ("À configurer", "Setup required", "Configuración pendiente"),
    ("Délai désactivé", "Timeout disabled", "Tiempo desactivado"),
    ("Opération en cours…", "Working…", "Operación en curso…"),
    ("Enregistré.", "Saved.", "Guardado."),
    ("Opération refusée.", "Operation refused.", "Operación rechazada."),
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
];
