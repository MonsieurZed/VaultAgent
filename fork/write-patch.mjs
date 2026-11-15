/* Modifier le CipherView en place conserve les passkeys et les champs non ciblés. */
const UUID = /^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/i;
const MAX_URIS = 20;
const MAX_URI_LENGTH = 2048;

/**
 * Indique si une adresse peut être ajoutée à une fiche : HTTPS sans identifiants, requête ni fragment.
 * @param {unknown} value Adresse proposée.
 * @returns {boolean} Vrai si l'adresse est acceptée.
 */
function httpsAddress(value) {
  if (typeof value !== 'string' || !value || value.length > MAX_URI_LENGTH) return false;
  try { const url = new URL(value); return url.protocol === 'https:' && !url.username && !url.password && !url.search && !url.hash; } catch { return false; }
}

/**
 * Indique si une adresse peut être désignée pour retrait : toute chaîne non vide, quel que soit son schéma.
 * @param {unknown} value Adresse proposée.
 * @returns {boolean} Vrai si l'adresse est acceptée.
 */
function namedAddress(value) {
  return typeof value === 'string' && !!value && value.length <= MAX_URI_LENGTH;
}

/**
 * Vérifie qu'une fiche est une connexion et qu'une liste d'adresses est bien formée.
 * @param {object} view Fiche à modifier.
 * @param {unknown} value Liste d'adresses.
 * @param {(address: unknown) => boolean} accepted Contrôle de chaque adresse.
 * @returns {void}
 */
function checkAddresses(view, value, accepted) {
  if (view.type !== 1 || !view.login || !Array.isArray(value) || !value.length || value.length > MAX_URIS || !value.every(accepted)) throw new Error('invalid-patch');
}

/**
 * Retire des adresses d'une connexion ; chacune doit exister, les autres entrées restent intactes.
 * @param {object} view Fiche à modifier.
 * @param {unknown} value Liste d'adresses à retirer, telles qu'enregistrées.
 * @returns {void}
 */
function removeAddresses(view, value) {
  checkAddresses(view, value, namedAddress);
  const current = view.login.uris || [];
  const gone = new Set(value);
  if (![...gone].every(uri => current.some(entry => entry.uri === uri))) throw new Error('invalid-patch');
  view.login.uris = current.filter(entry => !gone.has(entry.uri));
}

/**
 * Ajoute des adresses à une connexion sans retirer ni modifier celles qui existent.
 * @param {object} view Fiche à modifier.
 * @param {unknown} value Liste d'adresses à ajouter.
 * @param {(uri: string) => object} makeUri Fabrique d'une entrée d'adresse adaptée à la fiche.
 * @returns {void}
 */
function addAddresses(view, value, makeUri) {
  checkAddresses(view, value, httpsAddress);
  const current = view.login.uris || [];
  const present = new Set(current.map(entry => entry.uri));
  const added = [...new Set(value)].filter(uri => !present.has(uri));
  if (current.length + added.length > MAX_URIS) throw new Error('invalid-patch');
  view.login.uris = [...current, ...added.map(makeUri)];
}

/**
 * Applique un patch contrôlé à une fiche, après vérification de la révision et du propriétaire.
 * Les retraits d'adresses passent avant les ajouts, pour remplacer une adresse en une seule écriture.
 * @param {object} view Fiche à modifier en place.
 * @param {string} expectedRevision Date de révision attendue.
 * @param {object} patch Champs à modifier : name, folderId, notes, username, password, totp, removeUris, addUris.
 * @param {string|null} organizationId Organisation de la fiche, ou null si personnelle.
 * @param {(uri: string) => object} makeUri Fabrique d'une entrée d'adresse ; par défaut un objet simple.
 * @returns {object} La fiche modifiée.
 */
export function applyWritePatch(view, expectedRevision, patch, organizationId = null, makeUri = uri => ({ uri, match: null })) {
  const allowed = ['name', 'folderId', 'notes', 'username', 'password', 'totp', 'removeUris', 'addUris'];
  if (!patch || typeof patch !== 'object' || Array.isArray(patch) || !Object.keys(patch).length || Object.keys(patch).some(k => !allowed.includes(k))) throw new Error('invalid-patch');
  if (!expectedRevision || new Date(view.revisionDate).toISOString() !== new Date(expectedRevision).toISOString()) throw new Error('revision-conflict');
  if ((view.organizationId ?? null) !== organizationId || view.isDeleted || view.isArchived || view.deletedDate || view.archivedDate || ![1, 2].includes(view.type)) throw new Error('item-refused');
  if ('removeUris' in patch) removeAddresses(view, patch.removeUris);
  if ('addUris' in patch) addAddresses(view, patch.addUris, makeUri);
  for (const [key, value] of Object.entries(patch)) {
    if (key === 'removeUris' || key === 'addUris') continue;
    if (key === 'folderId') { if (value !== null && !UUID.test(value)) throw new Error('invalid-patch'); }
    else if (typeof value !== 'string' || value.length > (key === 'notes' ? 12000 : 8192) || key === 'name' && (!value.trim() || value.length > 500 || /[\x00-\x1f]/.test(value))) throw new Error('invalid-patch');
    if (['username', 'password', 'totp'].includes(key)) {
      if (view.type !== 1 || !view.login) throw new Error('item-refused');
      view.login[key] = value;
    } else view[key] = value;
  }
  return view;
}
