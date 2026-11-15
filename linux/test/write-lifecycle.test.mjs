import { test } from 'node:test';
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { Session } from '../session.mjs';
import { applyWritePatch } from '../../fork/write-patch.mjs';

const itemId = '00000000-0000-4000-8000-000000000001';
const folderId = '00000000-0000-4000-8000-000000000010';
const otherFolderId = '00000000-0000-4000-8000-000000000011';
const organizationId = '00000000-0000-4000-8000-000000000020';
const collectionId = '00000000-0000-4000-8000-000000000021';
const revision = '2026-11-08T00:00:00.000Z';
const next = '2026-11-08T00:05:00.000Z';
const operation = suffix => '00000000-0000-4000-8000-0000000001' + String(suffix).padStart(2, '0');
const secret = 'Fictif-seulement-42!';

function fixture({ shared = false, denied = false } = {}) {
  const disk = {}; const calls = []; let writes = 0;
  const folders = [{ id: folderId, name: 'Exemples' }, { id: otherFolderId, name: 'Autres' }];
  const items = [{ id: itemId, type: 1, name: 'Fiche factice', folderId, revisionDate: revision, organizationId: shared ? organizationId : null, collectionIds: shared ? [collectionId] : [],
    notes: secret, login: { username: 'user@example.invalid', password: secret, uris: [{ uri: 'https://one.example.test', match: null }, { uri: 'http://two.example.test', match: null }] } }];
  const store = { key: randomBytes(32), account: { profile: 'fake', timeoutMinutes: 15 }, async load(name, fallback) { return structuredClone(disk[name] ?? fallback); }, async save(name, value) { disk[name] = structuredClone(value); } };
  const engine = { alive: true, kill() {}, async request(request) {
    calls.push(request.op === 'call' ? request.method + ' ' + request.path : request.op);
    if (request.op === 'sharing-check') { if (denied) throw new Error('refused'); return { allowed: true }; }
    if (request.op === 'safe-update') {
      ++writes; const item = items.find(i => i.id === request.itemId); applyWritePatch(item, request.expectedRevisionDate, request.patch);
      item.revisionDate = next; return { id: item.id };
    }
    const { method, path, body } = request;
    if (path === '/sync') return {};
    if (path === '/list/object/items') return { data: structuredClone(items.filter(entry => !entry.deletedDate)) };
    if (path === '/list/object/items?trash=true') return { data: structuredClone(items.filter(entry => entry.deletedDate)) };
    if (path === '/list/object/folders') return { data: structuredClone(folders) };
    const item = path.match(/^\/(?:object|restore)\/item\/(.+)$/) && items.find(i => i.id === path.split('/').pop());
    if (method === 'GET' && item) return structuredClone(item);
    if (method === 'DELETE' && item) { ++writes; item.deletedDate = next; item.revisionDate = next; return {}; }
    if (method === 'POST' && path.startsWith('/restore/item/') && item) { ++writes; delete item.deletedDate; item.revisionDate = next; return structuredClone(item); }
    const folder = path.startsWith('/object/folder/') && folders.find(f => f.id === path.split('/').pop());
    if (method === 'PUT' && folder) { ++writes; folder.name = body.name; return structuredClone(folder); }
    if (method === 'DELETE' && folder) { ++writes; folders.splice(folders.indexOf(folder), 1); for (const entry of items) if (entry.folderId === folder.id) entry.folderId = null; return {}; }
    throw new Error('mock-route');
  } };
  const session = new Session(store); session.engine = engine; session.touch();
  return { session, disk, items, folders, calls, writes: () => writes };
}
const trash = (n = 1, extra = {}) => ({ operationId: operation(n), action: 'delete', itemId, expectedRevisionDate: revision, ...extra });

test('la corbeille marque la fiche supprimée, vérifie par relecture et reste idempotente', async () => {
  const f = fixture(); const request = trash(); const receipt = await f.session.write(request);
  assert.equal(receipt.verified, true); assert.equal(receipt.id, itemId); assert.equal(receipt.revisionDate, next);
  assert.ok(f.items[0].deletedDate); assert.ok(!JSON.stringify(receipt).includes(secret)); assert.ok(!JSON.stringify(f.disk).includes(secret));
  assert.equal((await f.session.write(request)).replayed, true); assert.equal(f.writes(), 1);
  await assert.rejects(f.session.write({ ...request, expectedRevisionDate: next }), /operation-conflict/);
  assert.ok(!f.calls.some(call => call.includes('permanent')));
});

test('la restauration exige la fiche en corbeille et la révision lue', async () => {
  const f = fixture(); await assert.rejects(f.session.write({ ...trash(2), action: 'restore' }), /item-refused/);
  await f.session.write(trash(3));
  await assert.rejects(f.session.write({ ...trash(4), action: 'restore' }), /revision-conflict/);
  const receipt = await f.session.write({ ...trash(5), action: 'restore', expectedRevisionDate: next });
  assert.equal(receipt.verified, true); assert.equal(f.items[0].deletedDate, undefined); assert.equal(f.items[0].login.password, secret);
  await assert.rejects(f.session.write(trash(6)), /revision-conflict/);
});

test('la corbeille refuse une fiche déjà supprimée, une révision périmée ou un mauvais propriétaire', async () => {
  const f = fixture();
  await assert.rejects(f.session.write(trash(7, { expectedRevisionDate: '2026-01-01T00:00:00.000Z' })), /revision-conflict/);
  await assert.rejects(f.session.write(trash(8, { organizationId })), /item-refused/);
  await f.session.write(trash(9)); await assert.rejects(f.session.write(trash(10, { expectedRevisionDate: next })), /item-refused/);
  assert.equal(f.writes(), 1);
});

test('une fiche commune exige son organisation et le droit de la collection', async () => {
  const shared = fixture({ shared: true });
  await assert.rejects(shared.session.write(trash(11)), /item-refused/);
  assert.equal((await shared.session.write(trash(12, { organizationId }))).verified, true); assert.ok(shared.calls.includes('sharing-check'));
  const denied = fixture({ shared: true, denied: true });
  await assert.rejects(denied.session.write(trash(13, { organizationId })), /refused/); assert.equal(denied.writes(), 0);
});

test('renommer un dossier vérifie le nom et refuse un doublon ou un dossier inconnu', async () => {
  const f = fixture(); const rename = (n, id, name) => ({ operationId: operation(n), action: 'update-folder', folderId: id, item: { name } });
  assert.equal((await f.session.write(rename(14, folderId, 'Renommé'))).verified, true); assert.equal(f.folders[0].name, 'Renommé');
  await assert.rejects(f.session.write(rename(15, folderId, 'Autres')), /duplicate-folder/);
  await assert.rejects(f.session.write(rename(16, '00000000-0000-4000-8000-0000000000ff', 'X')), /folder-missing/);
  await assert.rejects(f.session.write(rename(17, folderId, '  ')), /invalid-write/);
  assert.equal(f.writes(), 1);
});

test('supprimer un dossier détache ses fiches sans les supprimer', async () => {
  const f = fixture(); const request = { operationId: operation(18), action: 'delete-folder', folderId };
  const receipt = await f.session.write(request); assert.equal(receipt.verified, true); assert.equal(receipt.id, folderId);
  assert.equal(f.folders.length, 1); assert.equal(f.items[0].folderId, null); assert.equal(f.items[0].deletedDate, undefined);
  assert.equal((await f.session.write(request)).replayed, true); assert.equal(f.writes(), 1);
  await assert.rejects(f.session.write({ ...request, operationId: operation(19) }), /folder-missing/);
});

test('les formes mal construites sont refusées avant toute requête au moteur', async () => {
  const f = fixture(); const calls = f.calls.length;
  for (const bad of [
    { ...trash(20), patch: { name: 'X' } }, { ...trash(21), item: { name: 'X' } }, { ...trash(22), folderId }, { ...trash(23), itemId: 'pas-un-uuid' },
    { ...trash(24), expectedRevisionDate: undefined }, { ...trash(25), permanent: true },
    { operationId: operation(26), action: 'delete-folder' }, { operationId: operation(27), action: 'delete-folder', folderId, itemId },
    { operationId: operation(28), action: 'delete-folder', folderId, expectedRevisionDate: revision },
    { operationId: operation(29), action: 'update-folder', folderId }, { operationId: operation(30), action: 'update-folder', folderId, item: { name: 'X', extra: 1 } },
    { operationId: operation(31), action: 'create-folder', folderId, item: { name: 'X' } }
  ]) await assert.rejects(f.session.write(bad), /invalid-write/);
  assert.equal(f.calls.length, calls);
});

test('verrouillage et autorisation restent exigés pour le cycle de vie', async () => {
  const f = fixture();
  await assert.rejects(f.session.write(trash(32), () => { throw new Error('unauthorized'); }), /unauthorized/); assert.equal(f.writes(), 0);
  f.session.lock(); await assert.rejects(f.session.write(trash(33)), /locked/);
});

test('l\'aperçu d\'une fiche donne adresses et présence des champs sans aucune valeur, la corbeille est listée', async () => {
  const f = fixture(); const preview = await f.session.writePreview({ itemId });
  assert.deepEqual(preview.uris, ['https://one.example.test', 'http://two.example.test']);
  assert.deepEqual(preview.has, { username: true, password: true, totp: false, notes: true }); assert.equal(preview.deleted, false);
  assert.ok(!JSON.stringify(preview).includes(secret) && !JSON.stringify(preview).includes('user@example.invalid'));
  await f.session.write(trash(34));
  const list = await f.session.writePreview({}); assert.deepEqual(list.items, []); assert.equal(list.trash[0].id, itemId);
  const trashed = await f.session.writePreview({ itemId }); assert.equal(trashed.deleted, true); assert.equal(trashed.revisionDate, next);
});

test('removeUris retire exactement les adresses nommées, y compris non HTTPS, et se combine avec addUris', async () => {
  const f = fixture(); const base = { action: 'update', itemId, expectedRevisionDate: revision };
  const receipt = await f.session.write({ ...base, operationId: operation(35), patch: { removeUris: ['http://two.example.test'], addUris: ['https://two.example.test'] } });
  assert.equal(receipt.verified, true);
  assert.deepEqual(f.items[0].login.uris.map(entry => entry.uri), ['https://one.example.test', 'https://two.example.test']);
  assert.equal(f.items[0].login.password, secret); assert.equal(f.items[0].name, 'Fiche factice');
  await assert.rejects(f.session.write({ ...base, expectedRevisionDate: next, operationId: operation(36), patch: { removeUris: ['https://absent.example.test'] } }), /invalid-patch/);
  await assert.rejects(f.session.write({ ...base, expectedRevisionDate: next, operationId: operation(37), patch: { removeUris: [] } }), /invalid-patch/);
  const emptied = await f.session.write({ ...base, expectedRevisionDate: next, operationId: operation(38), patch: { removeUris: ['https://one.example.test', 'https://two.example.test'] } });
  assert.equal(emptied.verified, true); assert.deepEqual(f.items[0].login.uris, []);
});
