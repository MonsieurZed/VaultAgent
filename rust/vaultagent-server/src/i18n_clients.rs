//! Textes de l’onglet « Clients » : guide, demandes d’association, code et appareils.

/// Entrées (français, anglais, espagnol).
pub const ENTRIES: &[(&str, &str, &str)] = &[
    (
        "Comparez le code avec celui affiché dans le terminal du client avant d’autoriser. Le client sera associé au coffre.",
        "Compare the code with the one shown in the client terminal before approving. The client will be paired with the vault.",
        "Compara el código con el que aparece en el terminal del cliente antes de autorizar. El cliente se asociará a la bóveda.",
    ),
    (
        "Ce PC deviendra approbateur du coffre.",
        "This PC will become an approver for the vault.",
        "Este equipo será aprobador de la bóveda.",
    ),
    (
        "Demandes d’association",
        "Pairing requests",
        "Solicitudes de asociación",
    ),
    ("Autoriser sur", "Allow on", "Permitir en"),
    (
        "Toujours accepter les demandes de ce client",
        "Always accept requests from this client",
        "Aceptar siempre solicitudes de este cliente",
    ),
    (
        "À quoi sert un client ?",
        "What is a client for?",
        "¿Para qué sirve un cliente?",
    ),
    (
        "Ce n’est pas un autre compte du coffre : c’est le petit programme qui remplit les champs là où votre agent travaille.",
        "It is not another vault account: it is the small program that fills fields wherever your agent works.",
        "No es otra cuenta de la bóveda: es el pequeño programa que rellena campos donde trabaja tu agente.",
    ),
    (
        "Installer sur l’appareil utilisé par l’agent",
        "Install on the device used by the agent",
        "Instalar en el equipo que usa el agente",
    ),
    (
        "Client Windows · sans service",
        "Windows client · no service",
        "Cliente Windows · sin servicio",
    ),
    (
        "Client Linux · autonome",
        "Linux client · standalone",
        "Cliente Linux · autónomo",
    ),
    (
        "Créer un code ci-dessous et le saisir dans le programme du client.",
        "Create a code below and enter it in the client program.",
        "Crea un código abajo e introdúcelo en el programa del cliente.",
    ),
    (
        "Cliquer sur Autoriser les clients en haut de cette page pour permettre le remplissage.",
        "Click Authorize clients at the top of this page to allow filling.",
        "Pulsa Autorizar clientes al principio de esta página para permitir el rellenado.",
    ),
    (
        "Le client ne conserve pas votre mot de passe maître. Il garde seulement une autorisation que vous pouvez révoquer.",
        "The client does not store your master password. It only keeps an authorization that you can revoke.",
        "El cliente no guarda tu contraseña maestra. Solo conserva una autorización que puedes revocar.",
    ),
    (
        "Vos clients Windows et Linux",
        "Your Windows and Linux clients",
        "Tus clientes Windows y Linux",
    ),
    (
        "Associez chaque appareil ou agent. Un client peut recevoir une enveloppe chiffrée, jamais ouvrir ou configurer le coffre.",
        "Pair each device or agent. A client can receive an encrypted envelope, but cannot unlock or configure the vault.",
        "Asocia cada equipo o agente. Un cliente puede recibir un sobre cifrado, pero no desbloquear ni configurar la bóveda.",
    ),
    ("Nom du client", "Client name", "Nombre del cliente"),
    (
        "Créer un code d’association",
        "Create a pairing code",
        "Crear un código de asociación",
    ),
    (
        "Saisissez ce code dans le client. Usage unique, valable cinq minutes. Ne le collez pas dans un chat.",
        "Enter this code in the client. Single use, valid for five minutes. Do not paste it in chat.",
        "Introduce el código en el cliente. Un solo uso, válido cinco minutos. No lo pegues en un chat.",
    ),
    ("Afficher le code", "Show code", "Mostrar el código"),
    ("Révoquer", "Revoke", "Revocar"),
    (
        "Aucun client associé.",
        "No paired clients.",
        "No hay clientes asociados.",
    ),
    ("Enregistrer", "Save", "Guardar"),
    ("Valable jusqu’à", "Valid until", "Válido hasta"),
    ("Retour aux clients", "Back to clients", "Volver a los clientes"),
    ("Code d’association", "Pairing code", "Código de asociación"),
];
