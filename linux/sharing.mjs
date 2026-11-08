/* Une fiche appartient a l'organisation native : aucune copie entre comptes. */
import { isDeepStrictEqual } from 'node:util';
import { createHmac } from 'node:crypto';
const uuid = value => typeof value === 'string' && /^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/i.test(value);
const fields = ['name', 'type', 'notes', 'login', 'secureNote', 'card', 'identity', 'sshKey', 'fields', 'passwordHistory'];
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().filter(key => value[key] != null).map(key => [key, canonical(value[key])]));
  return value;
}
export async function sharingInfo(session, authorize) {
  authorize(); const generation = session.generation; const engine = session.access(generation);
  await engine.request({ op: 'call', method: 'POST', path: '/sync' });
  authorize(); session.access(generation); const result = await engine.request({ op: 'sharing-info' });
  authorize(); session.access(generation); session.touch(); return { organizations: result };
}
export async function shareItem(session, input, authorize) {
  if (!input || Object.keys(input).some(key => !['action', 'itemId', 'organizationId', 'collectionIds', 'operationId', 'expectedRevisionDate', 'confirmOwnershipTransfer', 'name'].includes(key)) || !['preview', 'apply', 'create-collection'].includes(input.action) || !uuid(input.organizationId)) throw new Error('invalid-share');
  if (input.action !== 'create-collection' && (!uuid(input.itemId) || !Array.isArray(input.collectionIds) || !input.collectionIds.length || input.collectionIds.length > 32 || input.collectionIds.some(id => !uuid(id)))) throw new Error('invalid-share');
  if (input.action === 'create-collection' && (typeof input.name !== 'string' || !input.name.trim() || input.name.length > 200 || /[\x00-\x1f]/.test(input.name))) throw new Error('invalid-share');
  if (input.action !== 'preview' && !uuid(input.operationId)) throw new Error('invalid-share');
  authorize(); const generation = session.generation; const engine = session.access(generation);
  const call = (method, path, body) => engine.request({ op: 'call', method, path, body });
  const ledger = input.action === 'preview' ? {} : await session.store.load('share-receipts.json', {});
  const key = session.store.account.profile + ':' + input.operationId;
  const fingerprint = createHmac('sha256', session.store.key).update(JSON.stringify(canonical(input))).digest('hex');
  if (input.action !== 'preview' && ledger[key]) {
    if (ledger[key].fingerprint !== fingerprint) throw new Error('operation-conflict');
    if (!ledger[key].receipt) throw new Error('share-indeterminate');
    authorize(); session.access(generation); return { ...ledger[key].receipt, replayed: true };
  }
  await call('POST', '/sync'); authorize(); session.access(generation);
  const organizations = await engine.request({ op: 'sharing-info' });
  const org = organizations.find(org => org.id === input.organizationId);
  if (!org) throw new Error('organization-refused');
  let original;
  if (input.action !== 'create-collection') {
    await engine.request({ op: 'sharing-check', organizationId: input.organizationId, collectionIds: input.collectionIds });
    original = await call('GET', '/object/item/' + input.itemId);
    if (!original || original.organizationId || original.deletedDate || original.archivedDate || ![1, 2].includes(original.type) || original.attachments?.length) throw new Error('item-refused');
    if (input.action === 'preview') {
      authorize(); session.access(generation); session.touch();
      return { itemId: original.id, name: original.name, expectedRevisionDate: original.revisionDate,
        organizationId: org.id, organizationName: org.name, collectionIds: input.collectionIds,
        transfersOwnership: true, requiresConfirmation: true };
    }
    if (input.confirmOwnershipTransfer !== true || input.expectedRevisionDate !== original.revisionDate) throw new Error('revision-conflict');
  }
  if (Object.keys(ledger).length >= 10000) throw new Error('receipt-capacity');
  if (input.action === 'create-collection' && org.collections.some(collection => collection.name === input.name.trim())) throw new Error('duplicate-collection');
  authorize(); session.access(generation); ledger[key] = { fingerprint }; await session.store.save('share-receipts.json', ledger);
  try {
    let id;
    if (input.action === 'create-collection') id = (await call('POST', '/object/org-collection', { name: input.name.trim(), organizationId: input.organizationId })).id;
    else { id = original.id; await call('POST', '/move/' + id + '/' + input.organizationId, input.collectionIds); }
    if (!uuid(id)) throw new Error('verification-failed');
    let verified = false;
    for (let attempt = 0; attempt < 5; ++attempt) {
      authorize(); session.access(generation); await call('POST', '/sync');
      if (input.action === 'create-collection') {
        const info = await engine.request({ op: 'sharing-info' });
        verified = info.some(org => org.id === input.organizationId && org.collections.some(collection => collection.id === id && collection.name === input.name.trim()));
      } else {
        const saved = await call('GET', '/object/item/' + id);
        verified = saved.organizationId === input.organizationId && isDeepStrictEqual([...(saved.collectionIds || [])].sort(), [...new Set(input.collectionIds)].sort()) && fields.every(field => isDeepStrictEqual(canonical(saved[field] ?? null), canonical(original[field] ?? null)));
      }
      if (verified) break; await new Promise(resolve => setTimeout(resolve, 200));
    }
    if (!verified) throw new Error('verification-failed');
    authorize(); session.access(generation);
    const receipt = { id, operationId: input.operationId, action: input.action, organizationId: input.organizationId, verified: true };
    ledger[key] = { fingerprint, receipt }; await session.store.save('share-receipts.json', ledger); session.touch(); return receipt;
  } catch { throw new Error('share-indeterminate'); }
}
