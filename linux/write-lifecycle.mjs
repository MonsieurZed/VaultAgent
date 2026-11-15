/* Cycle de vie des fiches et dossiers : corbeille, restauration, renommage et suppression de dossier. */
import { uuid, keys, revision, name, visible } from './write-checks.mjs';
import { openLedger, replayed, ensureCapacity, recordIntent, recordReceipt } from './write-ledger.mjs';

export const LIFECYCLE_ACTIONS = ['delete', 'restore', 'update-folder', 'delete-folder'];
const ITEM_ACTIONS = ['delete', 'restore'];
const ATTEMPTS = 5;
const RETRY_MS = 200;

/**
 * Valide la forme d'une requête de cycle de vie, avant toute requête au moteur.
 * @param {object} input Requête d'écriture.
 * @returns {void}
 */
export function validateLifecycle(input) {
  const itemAction = ITEM_ACTIONS.includes(input.action);
  if (input.item && input.action !== 'update-folder' || input.patch) throw new Error('invalid-write');
  if (itemAction) {
    if (!uuid(input.itemId) || input.folderId || !revision(input.expectedRevisionDate)) throw new Error('invalid-write');
    return;
  }
  if (!uuid(input.folderId) || input.itemId || input.expectedRevisionDate || input.organizationId) throw new Error('invalid-write');
  if (input.action === 'update-folder' && (!keys(input.item, ['name']) || !name(input.item.name, 250))) throw new Error('invalid-write');
}

/**
 * Vérifie la fiche ciblée avant corbeille ou restauration : état, révision, propriétaire et droits.
 * @param {object} session Session du coffre.
 * @param {Function} call Appel au moteur (method, path, body).
 * @param {object} input Requête d'écriture.
 * @returns {Promise<void>}
 */
async function checkItem(session, call, input) {
  const item = visible(await call('GET', '/object/item/' + input.itemId));
  const trashed = !!item.deletedDate;
  if (trashed !== (input.action === 'restore')) throw new Error('item-refused');
  if (new Date(item.revisionDate).toISOString() !== new Date(input.expectedRevisionDate).toISOString()) throw new Error('revision-conflict');
  if ((item.organizationId ?? null) !== (input.organizationId ?? null)) throw new Error('item-refused');
  if (item.organizationId) await session.access(session.generation).request({ op: 'sharing-check', organizationId: item.organizationId, collectionIds: item.collectionIds || [] });
}

/**
 * Vérifie le dossier ciblé : il existe et, pour un renommage, aucun autre dossier ne porte ce nom.
 * @param {Function} call Appel au moteur (method, path, body).
 * @param {object} input Requête d'écriture.
 * @returns {Promise<void>}
 */
async function checkFolder(call, input) {
  const folders = (await call('GET', '/list/object/folders')).data;
  if (!folders.some(folder => folder.id === input.folderId)) throw new Error('folder-missing');
  if (input.action === 'update-folder' && folders.some(folder => folder.id !== input.folderId && folder.name === input.item.name)) throw new Error('duplicate-folder');
}

/**
 * Exécute l'écriture demandée.
 * @param {Function} call Appel au moteur (method, path, body).
 * @param {object} input Requête d'écriture.
 * @returns {Promise<unknown>} Réponse du moteur.
 */
function perform(call, input) {
  if (input.action === 'delete') return call('DELETE', '/object/item/' + input.itemId);
  if (input.action === 'restore') return call('POST', '/restore/item/' + input.itemId);
  if (input.action === 'update-folder') return call('PUT', '/object/folder/' + input.folderId, { name: input.item.name });
  return call('DELETE', '/object/folder/' + input.folderId);
}

/**
 * Relit le coffre pour confirmer l'effet de l'écriture.
 * @param {Function} call Appel au moteur (method, path, body).
 * @param {object} input Requête d'écriture.
 * @returns {Promise<object|null>} Reçu partiel si l'effet est constaté, sinon null.
 */
async function confirm(call, input) {
  if (ITEM_ACTIONS.includes(input.action)) {
    const saved = visible(await call('GET', '/object/item/' + input.itemId));
    return !!saved.deletedDate === (input.action === 'delete') ? { id: input.itemId, revisionDate: saved.revisionDate } : null;
  }
  const folders = (await call('GET', '/list/object/folders')).data;
  const present = folders.some(folder => folder.id === input.folderId && (input.action !== 'update-folder' || folder.name === input.item.name));
  return present === (input.action === 'update-folder') ? { id: input.folderId } : null;
}

/**
 * Exécute une opération de cycle de vie avec reçu idempotent et vérification par relecture.
 * @param {object} session Session du coffre.
 * @param {object} input Requête d'écriture validée.
 * @param {Function} authorize Contrôle d'autorisation, rappelé avant chaque étape.
 * @returns {Promise<object>} Reçu { id, operationId, action, verified, revisionDate? }.
 */
export async function saveLifecycle(session, input, authorize) {
  authorize(); const generation = session.generation; const engine = session.access(generation);
  const entry = await openLedger(session, input);
  const replay = replayed(entry);
  if (replay) { authorize(); session.access(generation); return replay; }
  ensureCapacity(entry);
  const call = (method, path, body) => engine.request({ op: 'call', method, path, body });
  await call('POST', '/sync'); authorize(); session.access(generation);
  if (ITEM_ACTIONS.includes(input.action)) await checkItem(session, call, input); else await checkFolder(call, input);
  authorize(); session.access(generation);
  await recordIntent(session, entry);
  try {
    authorize(); session.access(generation);
    await perform(call, input);
    let confirmed = null;
    for (let attempt = 0; attempt < ATTEMPTS && !confirmed; ++attempt) {
      authorize(); session.access(generation); confirmed = await confirm(call, input);
      if (!confirmed) await new Promise(resolve => setTimeout(resolve, RETRY_MS));
    }
    if (!confirmed) throw new Error('verification-failed');
    authorize(); session.access(generation);
    return await recordReceipt(session, entry, { ...confirmed, operationId: input.operationId, action: input.action, verified: true });
  } catch { throw new Error('write-indeterminate'); }
}
