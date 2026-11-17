//! Textes de la page « Aide » : ce que protège le remplissage, ajout d'un appareil, version.

/// Entrées (français, anglais, espagnol).
pub const ENTRIES: &[(&str, &str, &str)] = &[
    (
        "Ce que protège le remplissage",
        "What blind filling protects",
        "Qué protege el rellenado",
    ),
    (
        "Les mots de passe et codes TOTP sont déchiffrés dans le programme de l’appareil et remplis sans presse-papiers. L’agent reçoit seulement une confirmation.",
        "Passwords and TOTP codes are decrypted inside the device program and filled without the clipboard. The agent receives only a confirmation.",
        "Las contraseñas y códigos TOTP se descifran dentro del programa del dispositivo y se rellenan sin portapapeles. El agente solo recibe una confirmación.",
    ),
    (
        "Un agent qui contrôle entièrement le navigateur peut encore lire un champ rempli. Ce mécanisme évite l’affichage des secrets dans le flux normal ; il ne protège pas contre du code arbitraire malveillant.",
        "An agent with full browser control can still read filled fields. This avoids displaying secrets during normal use; it does not protect against arbitrary malicious code.",
        "Un agente con control total del navegador puede leer los campos rellenados. Esto evita mostrar secretos durante el uso normal; no protege contra código arbitrario malicioso.",
    ),
    (
        "Authentik protège l’administration. Chaque appareil possède une autorisation distincte, que vous pouvez retirer. Aucun accès brut aux secrets n’est proposé.",
        "Authentik protects administration. Each device has a separate authorization that you can remove. No raw-secret endpoint is provided.",
        "Authentik protege la administración. Cada dispositivo tiene una autorización independiente que puedes quitar. No se ofrece acceso a los secretos sin cifrar.",
    ),
    (
        "Ouvrir « Ajouter un appareil », installer le programme et créer un code.",
        "Open “Add a device”, install the program and create a code.",
        "Abre «Añadir un dispositivo», instala el programa y crea un código.",
    ),
    (
        "Saisir le code dans le programme de l’appareil.",
        "Enter the code in the device program.",
        "Introduce el código en el programa del dispositivo.",
    ),
    (
        "Valider la demande sur la page principale, puis cliquer sur Déverrouiller pour permettre le remplissage.",
        "Approve the request on the main page, then click Unlock to allow filling.",
        "Aprueba la solicitud en la página principal y pulsa Desbloquear para permitir el rellenado.",
    ),
    ("Version du serveur", "Server version", "Versión del servidor"),
];
