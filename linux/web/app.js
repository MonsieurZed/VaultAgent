/* Aucune valeur de coffre n'est chargee dans cette interface apres enregistrement. */
const $ = id => document.getElementById(id);
const translations = {
  'À quoi sert un client ?': ['What is a client for?', '¿Para qué sirve un cliente?'],
  'Ce n’est pas un autre compte du coffre : c’est le petit programme qui remplit les champs là où votre agent travaille.': ['It is not another vault account: it is the small program that fills fields wherever your agent works.', 'No es otra cuenta de la bóveda: es el pequeño programa que rellena campos donde trabaja tu agente.'],
  'Installer sur l’appareil utilisé par l’agent': ['Install on the device used by the agent', 'Instalar en el equipo que usa el agente'],
  'Client Windows · sans service': ['Windows client · no service', 'Cliente Windows · sin servicio'],
  'Client Linux · autonome': ['Linux client · standalone', 'Cliente Linux · autónomo'],
  'Créer un code ci-dessous et le saisir dans le programme du client.': ['Create a code below and enter it in the client program.', 'Crea un código abajo e introdúcelo en el programa del cliente.'],
  'Cliquer sur Autoriser les clients en haut de cette page pour permettre le remplissage.': ['Click Authorize clients at the top of this page to allow filling.', 'Pulsa Autorizar clientes al principio de esta página para permitir el rellenado.'],
  'Le client ne conserve pas votre mot de passe maître. Il garde seulement une autorisation que vous pouvez révoquer.': ['The client does not store your master password. It only keeps an authorization that you can revoke.', 'El cliente no guarda tu contraseña maestra. Solo conserva una autorización que puedes revocar.'],
  'Votre coffre, disponible sans votre PC': ['Your vault, available without your PC', 'Tu bóveda, disponible sin tu PC'],
  'Un coffre. Vos clients. Le contrôle reste à vous.': ['One vault. Your clients. You stay in control.', 'Una bóveda. Tus clientes. Tú mantienes el control.'],
  'Le serveur conserve le moteur du coffre. Les clients remplissent les champs sans afficher les secrets aux agents.': ['The server runs the vault engine. Clients fill fields without displaying secrets to agents.', 'El servidor mantiene el motor de la bóveda. Los clientes rellenan campos sin mostrar secretos a los agentes.'],
  'Autoriser les clients': ['Authorize clients', 'Autorizar clientes'], 'Verrouiller': ['Lock', 'Bloquear'],
  'Compte': ['Account', 'Cuenta'], 'Clients': ['Clients', 'Clientes'], 'Fonctionnement': ['How it works', 'Funcionamiento'],
  'Connecter votre coffre': ['Connect your vault', 'Conectar tu bóveda'],
  'À configurer une fois. Le mot de passe n’est jamais renvoyé par l’API.': ['Set up once. The API never returns the password.', 'Configura una vez. La API nunca devuelve la contraseña.'],
  'Adresse HTTPS du coffre': ['HTTPS vault address', 'Dirección HTTPS de la bóveda'],
  'L’adresse que vous ouvrez pour accéder à Bitwarden ou Vaultwarden.': ['The address you open to access Bitwarden or Vaultwarden.', 'La dirección que abres para acceder a Bitwarden o Vaultwarden.'],
  'Adresse email': ['Email address', 'Correo electrónico'],
  'Le compte de votre coffre, pas nécessairement celui du SSO.': ['Your vault account, which may differ from your SSO account.', 'La cuenta de tu bóveda, que puede ser distinta de tu cuenta SSO.'],
  'Mot de passe maître': ['Master password', 'Contraseña maestra'],
  'Déjà enregistré ? Laissez vide pour le conserver. Ne le transmettez pas dans le chat.': ['Already saved? Leave blank to keep it. Never send it in chat.', '¿Ya está guardada? Déjalo vacío para conservarla. Nunca la envíes por el chat.'],
  'Code 2FA · facultatif': ['2FA code · optional', 'Código 2FA · opcional'],
  'Seulement si la double authentification est activée sur ce coffre.': ['Only if two-factor authentication is enabled for this vault.', 'Solo si la autenticación de dos factores está activada en esta bóveda.'],
  'Après utilisation': ['After use', 'Después del uso'], 'Verrouiller automatiquement': ['Lock automatically', 'Bloquear automáticamente'], 'Délai en minutes': ['Timeout in minutes', 'Tiempo en minutos'],
  'Chaque utilisation autorisée prolonge le délai. Le redémarrage du serveur verrouille toujours le coffre.': ['Each authorized use extends the timeout. Restarting the server always locks the vault.', 'Cada uso autorizado prolonga el plazo. Reiniciar el servidor siempre bloquea la bóveda.'],
  'Vérifier et enregistrer': ['Verify and save', 'Verificar y guardar'],
  'Changer de compte révoque les clients associés. Windows Hello et les passkeys Windows ne sont pas fournis par ce serveur.': ['Changing accounts revokes paired clients. This server does not provide Windows Hello or Windows passkeys.', 'Cambiar de cuenta revoca los clientes asociados. Este servidor no proporciona Windows Hello ni passkeys de Windows.'],
  'Vos clients Windows et Linux': ['Your Windows and Linux clients', 'Tus clientes Windows y Linux'],
  'Associez chaque appareil ou agent. Un client peut recevoir une enveloppe chiffrée, jamais ouvrir ou configurer le coffre.': ['Pair each device or agent. A client can receive an encrypted envelope, but cannot unlock or configure the vault.', 'Asocia cada equipo o agente. Un cliente puede recibir un sobre cifrado, pero no desbloquear ni configurar la bóveda.'],
  'Nom du client': ['Client name', 'Nombre del cliente'], 'Créer un code d’association': ['Create a pairing code', 'Crear un código de asociación'],
  'Saisissez ce code dans le client. Usage unique, valable cinq minutes. Ne le collez pas dans un chat.': ['Enter this code in the client. Single use, valid for five minutes. Do not paste it in chat.', 'Introduce el código en el cliente. Un solo uso, válido cinco minutos. No lo pegues en un chat.'],
  'Afficher le code': ['Show code', 'Mostrar el código'], 'Révoquer': ['Revoke', 'Revocar'],
  'Ce que protège le remplissage': ['What blind filling protects', 'Qué protege el rellenado'],
  'Les mots de passe et codes TOTP sont déchiffrés dans le helper du client et remplis sans presse-papiers. L’agent reçoit seulement une confirmation.': ['Passwords and TOTP codes are decrypted inside the client helper and filled without the clipboard. The agent receives only a confirmation.', 'Las contraseñas y códigos TOTP se descifran dentro del helper del cliente y se rellenan sin portapapeles. El agente solo recibe una confirmación.'],
  'Un agent qui contrôle entièrement le navigateur peut encore lire un champ rempli. Ce mécanisme évite l’affichage des secrets dans le flux normal ; il ne protège pas contre du code arbitraire malveillant.': ['An agent with full browser control can still read filled fields. This avoids displaying secrets during normal use; it does not protect against arbitrary malicious code.', 'Un agente con control total del navegador puede leer los campos rellenados. Esto evita mostrar secretos durante el uso normal; no protege contra código arbitrario malicioso.'],
  'Authentik protège l’administration. Chaque client possède une autorisation distincte et révocable. Aucun accès brut aux secrets n’est proposé.': ['Authentik protects administration. Each client has a separate, revocable authorization. No raw-secret endpoint is provided.', 'Authentik protege la administración. Cada cliente tiene una autorización independiente y revocable. No se ofrece acceso a los secretos sin cifrar.'],
  'Accès privé LAN / VPN': ['Private LAN / VPN access', 'Acceso privado LAN / VPN'],
  'Coffre verrouillé': ['Vault locked', 'Bóveda bloqueada'], 'Coffre disponible pour les clients': ['Vault available to clients', 'Bóveda disponible para los clientes'],
  'À configurer': ['Setup required', 'Configuración pendiente'], 'Délai désactivé': ['Timeout disabled', 'Tiempo desactivado'],
  'Aucun client associé.': ['No paired clients.', 'No hay clientes asociados.'], 'Opération en cours…': ['Working…', 'Operación en curso…'], 'Enregistré.': ['Saved.', 'Guardado.'],
  'Opération refusée.': ['Operation refused.', 'Operación rechazada.'],
  'Connexion refusée. Vérifiez le mot de passe maître du coffre et le code 2FA éventuel.': ['Sign-in refused. Check the vault master password and 2FA code if required.', 'Inicio de sesión rechazado. Comprueba la contraseña maestra de la bóveda y el código 2FA si se requiere.'],
  'Authentification expirée. Rechargez la page pour vous reconnecter.': ['Authentication expired. Reload to sign in again.', 'Autenticación caducada. Recarga la página para volver a entrar.']
};
let language = localStorage.getItem('vaultagent-language') || 'fr'; let snapshot;
const t = source => language === 'fr' ? source : translations[source]?.[language === 'es' ? 1 : 0] || source;
function localize() { document.documentElement.lang = language; document.querySelectorAll('[data-t]').forEach(element => element.textContent = t(element.dataset.t)); if (snapshot) display(snapshot); }
$('language').value = language; $('language').addEventListener('change', event => { language = event.target.value; localStorage.setItem('vaultagent-language', language); localize(); });
document.querySelectorAll('[data-tab]').forEach(button => button.addEventListener('click', () => { document.querySelectorAll('[data-tab]').forEach(other => other.classList.toggle('selected', other === button)); document.querySelectorAll('.tab').forEach(tab => tab.hidden = tab.id !== button.dataset.tab); }));
async function api(path, data) {
  const response = await fetch('/api/admin/' + path, { method: data === undefined ? 'GET' : 'POST', credentials: 'same-origin', headers: { 'content-type': 'application/json' }, ...(data === undefined ? {} : { body: JSON.stringify(data) }) });
  if (!response.ok) { const error = await response.json().catch(() => ({})); throw new Error(response.status === 403 ? 'Authentification expirée. Rechargez la page pour vous reconnecter.' : error.error?.startsWith('login:') || error.error?.startsWith('unlock:') ? 'Connexion refusée. Vérifiez le mot de passe maître du coffre et le code 2FA éventuel.' : 'Opération refusée.'); }
  return response.json();
}
function display(state) {
  snapshot = state; $('state').textContent = t(!state.configured ? 'À configurer' : state.unlocked ? 'Coffre disponible pour les clients' : 'Coffre verrouillé');
  $('dot').textContent = state.unlocked ? '● ' : '○ '; $('expiration').textContent = state.unlocked ? state.expiresUtc ? new Date(state.expiresUtc).toLocaleTimeString(language) : t('Délai désactivé') : '';
  $('unlock').disabled = !state.configured; $('lock').disabled = !state.unlocked;
  $('devices').replaceChildren();
  for (const device of state.devices) {
    const row = document.createElement('div'); row.className = 'device'; const info = document.createElement('div'); const name = document.createElement('strong'); name.textContent = device.name; const date = document.createElement('small'); date.textContent = new Date(device.created).toLocaleString(language); info.append(name, date);
    const revoke = document.createElement('button'); revoke.className = 'quiet'; revoke.textContent = t('Révoquer'); revoke.addEventListener('click', () => action(revoke, () => api('revoke', { deviceId: device.id }))); row.append(info, revoke); $('devices').append(row);
  }
  if (!state.devices.length) { const empty = document.createElement('p'); empty.textContent = t('Aucun client associé.'); $('devices').append(empty); }
}
async function refresh(initial = false) {
  const state = await api('state'); display(state);
  if (initial && state.account) { $('server').value = state.account.server; $('email').value = state.account.email; $('automatic').checked = state.account.timeoutMinutes !== null; $('minutes').value = state.account.timeoutMinutes || 15; $('minutes').disabled = !$('automatic').checked; }
}
async function action(button, callback) { button.disabled = true; $('message').textContent = t('Opération en cours…'); try { await callback(); await refresh(); $('message').textContent = t('Enregistré.'); } catch (error) { $('message').textContent = t(error.message); } finally { button.disabled = false; } }
$('automatic').addEventListener('change', () => $('minutes').disabled = !$('automatic').checked);
$('account-form').addEventListener('submit', event => { event.preventDefault(); action(event.submitter, async () => { const data = { server: $('server').value.trim().replace(/\/$/, ''), email: $('email').value.trim(), password: $('password').value, code: $('code').value, timeoutMinutes: $('automatic').checked ? Number($('minutes').value) : null }; try { await api('configure', data); } finally { $('password').value = ''; $('code').value = ''; data.password = ''; data.code = ''; } }); });
$('unlock').addEventListener('click', () => action($('unlock'), async () => { try { await api('unlock', { code: $('unlock-code').value }); } finally { $('unlock-code').value = ''; } }));
$('lock').addEventListener('click', () => action($('lock'), () => api('lock', {})));
$('pair-form').addEventListener('submit', event => { event.preventDefault(); action(event.submitter, async () => { const value = await api('pairing', { name: $('device-name').value }); $('pair-code').value = value.code; $('pairing').hidden = false; $('show-pair').checked = false; $('pair-code').type = 'password'; setTimeout(() => { $('pair-code').value = ''; $('pairing').hidden = true; }, value.expiresInSeconds * 1000); }); });
$('show-pair').addEventListener('change', () => $('pair-code').type = $('show-pair').checked ? 'text' : 'password');
localize(); refresh(true).catch(error => $('message').textContent = t(error.message));
setInterval(() => refresh().catch(() => {}), 10000);
