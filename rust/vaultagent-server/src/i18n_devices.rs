//! Textes des appareils : liste, demandes d'ajout, page « Ajouter un appareil » et page du code.

/// Entrées (français, anglais, espagnol).
pub const ENTRIES: &[(&str, &str, &str)] = &[
    ("Appareils", "Devices", "Dispositivos"),
    ("Aucun appareil.", "No devices.", "No hay dispositivos."),
    ("validateur", "approver", "validador"),
    ("agent", "agent", "agente"),
    ("carte", "card", "tarjeta"),
    ("Sans demander", "Don’t ask", "Sin preguntar"),
    ("Enregistrer", "Save", "Guardar"),
    ("Retirer", "Remove", "Quitar"),
    ("ajouté le", "added on", "añadido el"),
    (
        "Il ne pourra plus rien remplir.",
        "It will no longer be able to fill anything.",
        "Ya no podrá rellenar nada.",
    ),
    ("Confirmer le retrait", "Confirm removal", "Confirmar la retirada"),
    ("Annuler", "Cancel", "Cancelar"),
    ("Précédent", "Previous", "Anterior"),
    ("Suivant", "Next", "Siguiente"),
    ("Ajouter", "Add", "Añadir"),
    ("Code à comparer", "Code to compare", "Código a comparar"),
    ("Ajouter un appareil", "Add a device", "Añadir un dispositivo"),
    (
        "Installez le programme sur l’appareil utilisé par l’agent, puis créez un code et saisissez-le dans ce programme.",
        "Install the program on the device used by the agent, then create a code and enter it in that program.",
        "Instala el programa en el dispositivo que usa el agente, crea un código e introdúcelo en ese programa.",
    ),
    (
        "Télécharger le programme",
        "Download the program",
        "Descargar el programa",
    ),
    (
        "Programme Windows · sans service",
        "Windows program · no service",
        "Programa Windows · sin servicio",
    ),
    (
        "Programme Linux · autonome",
        "Linux program · standalone",
        "Programa Linux · autónomo",
    ),
    ("Nom de l’appareil", "Device name", "Nombre del dispositivo"),
    (
        "Créer un code d’ajout",
        "Create an add code",
        "Crear un código de adición",
    ),
    (
        "Saisissez ce code dans le programme de l’appareil. Usage unique, valable cinq minutes. Ne le collez pas dans un chat.",
        "Enter this code in the device program. Single use, valid for five minutes. Do not paste it in chat.",
        "Introduce el código en el programa del dispositivo. Un solo uso, válido cinco minutos. No lo pegues en un chat.",
    ),
    ("Afficher le code", "Show code", "Mostrar el código"),
    ("Valable jusqu’à", "Valid until", "Válido hasta"),
    ("Code d’ajout", "Add code", "Código de adición"),
];
