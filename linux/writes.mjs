/* Écritures dédiées : aucun objet déchiffré ni empreinte de secret dans les reçus. */
import { isDeepStrictEqual } from 'node:util';
import { applyWritePatch } from '../fork/write-patch.mjs';
import { uuid, keys, active, visible, canonical } from './write-checks.mjs';
import { openLedger, replayed, ensureCapacity, recordIntent, recordReceipt } from './write-ledger.mjs';
import { LIFECYCLE_ACTIONS, validateLifecycle, saveLifecycle } from './write-lifecycle.mjs';

function comparable(item, passwordChanged) {
  const copy = structuredClone(item); delete copy.revisionDate;
  // Le serveur entretient l'historique et la date du mot de passe modifié.
  if (passwordChanged) { delete copy.passwordHistory; if (copy.login) delete copy.login.passwordRevisionDate; }
  for (const attachment of copy.attachments || []) delete attachment.url;
  for (const entry of copy.login?.uris || []) delete entry.uriChecksum;
  return canonical(copy);
}
const metadata = item => ({ id: item.id, name: item.name, type: item.type, folderId: item.folderId ?? null, revisionDate: item.revisionDate, organizationId: item.organizationId ?? null, collectionIds: item.collectionIds || [] });
const present = value => typeof value === 'string' && value.length > 0;
const detail = item => ({ ...metadata(item), deleted: !!item.deletedDate, uris: (item.login?.uris || []).map(entry => entry.uri), has: { username: present(item.login?.username), password: present(item.login?.password), totp: present(item.login?.totp), notes: present(item.notes) } });
export async function previewWrite(session, itemId, authorize) {
  const generation = session.generation; const engine = session.access(generation);
  await engine.request({ op: 'call', method: 'POST', path: '/sync' }); authorize(); session.access(generation);
  if (itemId === undefined) {
    const folders = (await engine.request({ op: 'call', method: 'GET', path: '/list/object/folders' })).data.map(folder => ({ id: folder.id, name: folder.name }));
    const listed = async suffix => (await engine.request({ op: 'call', method: 'GET', path: '/list/object/items' + suffix })).data.filter(item => !item.archivedDate && [1, 2].includes(item.type));
    const items = (await listed('')).filter(item => !item.deletedDate).map(metadata);
    const trash = (await listed('?trash=true')).filter(item => item.deletedDate).map(metadata);
    authorize(); session.access(generation); session.touch(); return { folders, items, trash };
  }
  if (!uuid(itemId)) throw new Error('invalid-item');
  const item = visible(await engine.request({ op: 'call', method: 'GET', path: '/object/item/' + itemId }));
  authorize(); session.access(generation); session.touch(); return detail(item);
}
function validate(input) {
  if (!keys(input, ['operationId', 'action', 'itemId', 'folderId', 'expectedRevisionDate', 'item', 'patch', 'organizationId']) || !uuid(input.operationId) || !['create', 'update', 'create-folder', ...LIFECYCLE_ACTIONS].includes(input.action) || input.organizationId != null && !uuid(input.organizationId)) throw new Error('invalid-write');
  if (LIFECYCLE_ACTIONS.includes(input.action)) validateLifecycle(input);
  else if (input.folderId) throw new Error('invalid-write');
  else if (input.action === 'create-folder') {
    if (input.itemId || input.patch || input.expectedRevisionDate || !keys(input.item, ['name']) || typeof input.item.name !== 'string' || !input.item.name.trim() || input.item.name.length > 250 || /[\x00-\x1f]/.test(input.item.name)) throw new Error('invalid-write');
  } else if (input.action === 'create') {
    const item = input.item;
    if (input.itemId || input.patch || input.expectedRevisionDate || !keys(item, ['type', 'name', 'folderId', 'notes', 'login', 'organizationId', 'collectionIds']) || ![1, 2].includes(item.type) || typeof item.name !== 'string' || !item.name.trim() || item.name.length > 500 || /[\x00-\x1f]/.test(item.name) || item.folderId != null && !uuid(item.folderId)) throw new Error('invalid-write');
    if (item.organizationId != null && (!uuid(item.organizationId) || !Array.isArray(item.collectionIds) || !item.collectionIds.length || item.collectionIds.length > 32 || item.collectionIds.some(id => !uuid(id))) || !item.organizationId && item.collectionIds?.length) throw new Error('invalid-write');
    if (item.notes !== undefined && (typeof item.notes !== 'string' || item.notes.length > 12000)) throw new Error('invalid-write');
    if (item.type === 1) {
      if (!keys(item.login, ['username', 'password', 'totp', 'uris'])) throw new Error('invalid-write');
      for (const field of ['username', 'password', 'totp']) if (item.login[field] !== undefined && (typeof item.login[field] !== 'string' || item.login[field].length > 8192)) throw new Error('invalid-write');
      if (item.login.uris !== undefined && (!Array.isArray(item.login.uris) || item.login.uris.length > 20 || item.login.uris.some(entry => {
        if (!keys(entry, ['uri']) || typeof entry.uri !== 'string' || entry.uri.length > 2048) return true;
        try { const url = new URL(entry.uri); return url.protocol !== 'https:' || !!url.username || !!url.password || !!url.search || !!url.hash; } catch { return true; }
      }))) throw new Error('invalid-write');
    } else if (item.login !== undefined) throw new Error('invalid-write');
  } else {
    if (input.item || !uuid(input.itemId) || typeof input.expectedRevisionDate !== 'string' || !Number.isFinite(Date.parse(input.expectedRevisionDate))) throw new Error('invalid-write');
    // Validation commune au moteur, avant toute requête d'édition.
    const removable = Array.isArray(input.patch?.removeUris) ? input.patch.removeUris.filter(uri => typeof uri === 'string').map(uri => ({ uri })) : [];
    applyWritePatch({ type: 1, login: { uris: removable }, revisionDate: input.expectedRevisionDate, organizationId: input.organizationId }, input.expectedRevisionDate, input.patch, input.organizationId ?? null);
  }
}
export async function saveWrite(session, input, authorize) {
  validate(input); authorize();
  if (LIFECYCLE_ACTIONS.includes(input.action)) return saveLifecycle(session, input, authorize);
  const generation = session.generation; const engine = session.access(generation);
  const entry = await openLedger(session, input);
  const replay = replayed(entry);
  if (replay) { authorize(); session.access(generation); return replay; }
  ensureCapacity(entry);
  const call = (method, path, body) => engine.request({ op: 'call', method, path, body });
  await call('POST', '/sync'); authorize(); session.access(generation);
  let expected; let before; let existingFolder;
  if (input.action === 'update') {
    expected = active(await call('GET', '/object/item/' + input.itemId));
    before = structuredClone(expected);
    applyWritePatch(expected, input.expectedRevisionDate, input.patch, input.organizationId ?? null);
  } else if (input.action === 'create') {
    // Ne pas confondre un nom déjà présent avec une autorisation d'écrasement.
    const existing = (await call('GET', '/list/object/items')).data;
    if (existing.some(item => !item.deletedDate && item.name === input.item.name && (item.folderId ?? null) === (input.item.folderId ?? null) && (item.organizationId ?? null) === (input.item.organizationId ?? null))) throw new Error('duplicate-item');
    if (input.item.organizationId) await engine.request({ op: 'sharing-check', organizationId: input.item.organizationId, collectionIds: input.item.collectionIds });
  } else {
    const existing = (await call('GET', '/list/object/folders')).data.filter(folder => folder.name === input.item.name);
    if (existing.length > 1) throw new Error('duplicate-folder'); existingFolder = existing[0];
  }
  const folderId = input.action === 'create' ? input.item.folderId : input.action === 'update' ? input.patch.folderId : null;
  if (folderId != null && !(await call('GET', '/list/object/folders')).data.some(folder => folder.id === folderId)) throw new Error('folder-missing');
  authorize(); session.access(generation);
  await recordIntent(session, entry);
  // À partir d'ici, une panne ou une réponse perdue peut avoir écrit : aucun retry aveugle.
  let id = input.itemId;
  try {
    authorize(); session.access(generation);
    if (input.action === 'create-folder') {
      id = existingFolder?.id ?? (await call('POST', '/object/folder', input.item)).id;
      if (!uuid(id)) throw new Error('verification-failed');
      let verified = false;
      for (let attempt = 0; attempt < 5; ++attempt) {
        authorize(); session.access(generation);
        verified = (await call('GET', '/list/object/folders')).data.some(folder => folder.id === id && folder.name === input.item.name);
        if (verified) break; await new Promise(resolve => setTimeout(resolve, 200));
      }
      if (!verified) throw new Error('verification-failed');
      authorize(); session.access(generation);
      return await recordReceipt(session, entry, { id, operationId: input.operationId, action: input.action, verified: true });
    }
    if (input.action === 'create') {
      const body = structuredClone(input.item); if (body.type === 2) body.secureNote = { type: 0 };
      id = (await call('POST', '/object/item', body)).id;
    } else await engine.request({ op: 'safe-update', itemId: id, expectedRevisionDate: input.expectedRevisionDate, patch: input.patch, organizationId: input.organizationId });
    if (!uuid(id)) throw new Error('invalid-result');
    let saved; let verified = false;
    for (let attempt = 0; attempt < 5; ++attempt) {
      authorize(); session.access(generation); saved = active(await call('GET', '/object/item/' + id));
      if (input.action === 'update') {
        const passwordChanged = 'password' in input.patch && (before.login?.password ?? '') !== input.patch.password;
        verified = isDeepStrictEqual(comparable(saved, passwordChanged), comparable(expected, passwordChanged));
        if (passwordChanged && before.login?.password) {
          const history = saved.passwordHistory || [];
          verified &&= history[0]?.password === before.login.password && isDeepStrictEqual(canonical(history.slice(1)), canonical((before.passwordHistory || []).slice(0, 4)));
        }
      } else verified = matchesCreated(saved, input.item);
      if (verified) break;
      await new Promise(resolve => setTimeout(resolve, 200));
    }
    if (!verified) throw new Error('verification-failed');
    authorize(); session.access(generation);
    return await recordReceipt(session, entry, { id, operationId: input.operationId, action: input.action, verified: true, revisionDate: saved.revisionDate });
  } catch { throw new Error('write-indeterminate'); }
}
function matchesCreated(saved, item) {
  if ((saved.organizationId ?? null) !== (item.organizationId ?? null) || !isDeepStrictEqual([...(saved.collectionIds || [])].sort(), [...(item.collectionIds || [])].sort())) return false;
  if (saved.type !== item.type || saved.name !== item.name || (saved.folderId ?? null) !== (item.folderId ?? null) || (saved.notes ?? '') !== (item.notes ?? '')) return false;
  if (item.type === 1) {
    for (const key of ['username', 'password', 'totp']) if ((saved.login?.[key] ?? '') !== (item.login[key] ?? '')) return false;
    if (!isDeepStrictEqual((saved.login?.uris || []).map(v => v.uri), (item.login.uris || []).map(v => v.uri))) return false;
  }
  return true;
}
