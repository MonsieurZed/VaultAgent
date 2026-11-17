//! Textes des messages d'erreur et de la page d'erreur : français, anglais, espagnol.

/// Entrées (français, anglais, espagnol).
pub const ENTRIES: &[(&str, &str, &str)] = &[
    (
        "Action refusée par le serveur.",
        "Action refused by the server.",
        "Acción rechazada por el servidor.",
    ),
    (
        "La page n’était plus à jour, l’action n’a pas été faite. Réessaie.",
        "The page was out of date, the action was not done. Try again.",
        "La página estaba desactualizada, la acción no se hizo. Inténtalo de nuevo.",
    ),
    (
        "Les données envoyées sont trop grosses, l’action n’a pas été faite.",
        "The data sent is too large, the action was not done.",
        "Los datos enviados son demasiado grandes, la acción no se hizo.",
    ),
    (
        "Action inconnue, rien n’a été fait.",
        "Unknown action, nothing was done.",
        "Acción desconocida, no se hizo nada.",
    ),
    (
        "Requête invalide, l’action n’a pas été faite.",
        "Invalid request, the action was not done.",
        "Solicitud no válida, la acción no se hizo.",
    ),
    (
        "Le coffre est verrouillé. Déverrouille-le d’abord.",
        "The vault is locked. Unlock it first.",
        "La bóveda está bloqueada. Desbloquéala primero.",
    ),
    (
        "Le coffre n’est pas configuré.",
        "The vault is not configured.",
        "La bóveda no está configurada.",
    ),
    (
        "Une valeur saisie est invalide.",
        "A value you entered is invalid.",
        "Un valor introducido no es válido.",
    ),
    (
        "Trop de demandes ou limite atteinte. Réessaie plus tard.",
        "Too many requests or limit reached. Try again later.",
        "Demasiadas solicitudes o límite alcanzado. Inténtalo más tarde.",
    ),
    ("Page introuvable.", "Page not found.", "Página no encontrada."),
    ("Accès refusé.", "Access denied.", "Acceso denegado."),
    ("Retour", "Back", "Volver"),
];
