import { test } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, sign, constants } from 'node:crypto';
import { UnlockRequests, decisionMessage } from '../unlock-requests.mjs';
import { createApp } from '../http.mjs';
import { spawnSync } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Store } from '../store.mjs';
import { Devices } from '../devices.mjs';

function fixture() {
  let now = 0; let opened = false; let unlocks = 0;
  const records = [{ id: 'device-a', name: 'PC factice' }, { id: 'device-b', name: 'Autre PC' }];
  const session = { generation: 0, store: { account: { profile: 'test', timeoutMinutes: 15 } },
    status: () => ({ configured: true, unlocked: opened }),
    async unlock() { ++unlocks; opened = true; }, lock() { opened = false; ++this.generation; } };
  const devices = { list: () => records, record: id => records.find(r => r.id === id), async setAutomatic(id, enabled) { records.find(r => r.id === id).autoUnlock = enabled; }, async clearAutomatic() { for (const row of records) row.autoUnlock = false; }, authenticate(header) { const id = header?.replace('Bearer ', ''); if (!records.some(r => r.id === id)) throw new Error('unauthorized'); return id; } };
  return { session, devices, queue: new UnlockRequests(session, devices, () => now), advance: ms => now += ms,
    records, unlocks: () => unlocks };
}
test('demande dédupliquée, seul accord explicite ouvre ; aucun renouvellement par sondage', async () => {
  const f = fixture(); const pending = await f.queue.request('device-a');
  assert.equal(pending.state, 'pending'); assert.equal(f.unlocks(), 0);
  assert.deepEqual(await f.queue.request('device-a'), pending); assert.equal(f.queue.current('device-b'), null);
  assert.equal(f.queue.list()[0].deviceName, 'PC factice');
  await f.queue.decide(pending.requestId, true);
  assert.equal(f.queue.current('device-a').state, 'approved'); assert.equal(f.unlocks(), 1);
  assert.equal((await f.queue.request('device-b')).unlocked, true); assert.equal(f.unlocks(), 1);
  await assert.rejects(f.queue.decide(pending.requestId, true), /request-expired/);
});
test('refus, expiration, révocation, changement de compte et verrouillage consomment la demande', async () => {
  const f = fixture(); const pending = await f.queue.request('device-a');
  await f.queue.decide(pending.requestId, false); assert.equal(f.unlocks(), 0);
  assert.equal((await f.queue.request('device-a')).state, 'denied');
  f.advance(300001); assert.equal(f.queue.current('device-a'), null);
  const next = await f.queue.request('device-a'); f.session.lock();
  await assert.rejects(f.queue.decide(next.requestId, true), /request-expired/);
  await f.queue.request('device-a'); f.records.splice(0, 1); assert.equal(f.queue.current('device-a'), null);
  const other = await f.queue.request('device-b'); f.session.store.account.profile = 'changed';
  await assert.rejects(f.queue.decide(other.requestId, true), /request-expired/);
});
test('révocation pendant approbation empêche de conserver la session ouverte', async () => {
  const f = fixture(); const pending = await f.queue.request('device-a'); let resume;
  f.session.unlock = () => new Promise(resolve => resume = resolve);
  const approving = f.queue.decide(pending.requestId, true);
  f.records.splice(0, 1); resume(); await assert.rejects(approving, /request-expired/);
  assert.equal(f.session.generation, 1);
});
test('API : demande client autorisée, décision réservée propriétaire avec origine exacte', async t => {
  const f = fixture(); const origin = 'https://vaultagent.example.test';
  const server = createApp({ ...f, origin, owner: 'Owner', trustedProxy: '127.0.0.1' });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); t.after(() => new Promise(resolve => server.close(resolve)));
  const call = (path, body = {}, headers = {}) => fetch('http://127.0.0.1:' + server.address().port + path,
    { method: 'POST', headers: { 'content-type': 'application/json', ...headers }, body: JSON.stringify(body) });
  assert.equal((await call('/api/agent/request-unlock')).status, 403);
  const auth = { authorization: 'Bearer device-a' };
  const pending = await (await call('/api/agent/request-unlock', {}, auth)).json(); assert.equal(pending.state, 'pending');
  const decision = { requestId: pending.requestId, approve: true };
  assert.equal((await call('/api/agent/decide-unlock', decision, auth)).status, 400);
  assert.equal((await call('/api/admin/decide-unlock', decision, { ...auth, origin })).status, 403);
  assert.equal((await call('/api/admin/decide-unlock', decision, { 'x-authentik-username': 'Owner', origin: 'https://wrong.test' })).status, 403);
  assert.equal(f.unlocks(), 0);
  assert.equal((await call('/api/admin/decide-unlock', decision, { 'x-authentik-username': 'Owner', origin })).status, 200);
  assert.equal(f.unlocks(), 1);
});
test('signature du PC approbateur, accord mémorisé sur Linux, désactivation et rejeu refusé', async () => {
  const f = fixture(); const origin = 'https://vaultagent.example.test';
  const { publicKey, privateKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  f.records[1].approverPublicKey = publicKey.export({ type: 'spki', format: 'der' }).toString('base64');
  const pending = await f.queue.request('device-a'); const challenge = f.queue.list()[0].challenge;
  const payload = { requestId: pending.requestId, approve: true, remember: true };
  const signed = (id, nonce, approve, remember) => sign('sha256', Buffer.from(decisionMessage(origin, id, nonce, approve, remember)), { key: privateKey, padding: constants.RSA_PKCS1_PSS_PADDING, saltLength: 32 }).toString('base64');
  payload.signature = signed(payload.requestId, challenge, true, true);
  f.queue.vaultId = 'other-vault'; await assert.rejects(f.queue.signedDecision('device-b', payload, origin), /approval-refused/); f.queue.vaultId = 'default';
  await assert.rejects(f.queue.signedDecision('device-a', payload, origin), /approver-required/);
  await assert.rejects(f.queue.signedDecision('device-b', { ...payload, approve: false }, origin), /approval-refused/);
  await f.queue.signedDecision('device-b', payload, origin);
  assert.equal(f.devices.record('device-a').autoUnlock, true);
  await assert.rejects(f.queue.signedDecision('device-b', payload, origin), /request-expired/);
  f.session.lock(); assert.equal((await f.queue.request('device-a')).unlocked, true); assert.equal(f.unlocks(), 2);
  const context = f.queue.context('device-b');
  const reset = { requestId: context.requestId, approve: false, remember: false, signature: signed(context.requestId, context.challenge, false, false) };
  await f.queue.signedDecision('device-b', reset, origin);
  assert.equal(f.devices.record('device-a').autoUnlock, false);
  await assert.rejects(f.queue.signedDecision('device-b', reset, origin), /request-expired/);
  f.session.lock(); assert.equal((await f.queue.request('device-a')).state, 'pending');
});
test('la politique automatique survit au redémarrage du stockage Linux', async t => {
  const directory = await mkdtemp(join(tmpdir(), 'vaultagent-unlock-test-')); t.after(() => rm(directory, { recursive: true, force: true }));
  const store = await new Store(directory).init(); store.account = { profile: 'test' }; await store.save('account.json', store.account);
  await store.updateDevices(() => [{ id: 'test-device', profile: 'test', name: 'Fictif' }]);
  const devices = new Devices(store); await devices.setAutomatic('test-device', true);
  const restored = new Devices(await new Store(directory).init()); assert.equal(restored.record('test-device').autoUnlock, true);
  await restored.setAutomatic('test-device', false); assert.equal(restored.record('test-device').autoUnlock, false);
});
test('signature .NET et validation Linux partagent le même message et RSA-PSS', { skip: process.platform !== 'win32' }, async () => {
  const f = fixture(); const origin = 'https://vaultagent.example.test'; const pending = await f.queue.request('device-a');
  const challenge = Buffer.from([251, 255, 254, 250, 255, 255]).toString('base64'); f.queue.entries.get('device-a').challenge = challenge;
  const result = spawnSync('pwsh.exe', ['-NoProfile', '-File', resolve('tests/approval-crypto.ps1'), '-Origin', origin, '-RequestId', pending.requestId, '-Challenge', challenge], { encoding: 'utf8', windowsHide: true });
  assert.equal(result.status, 0, result.stderr); const signed = JSON.parse(result.stdout);
  f.records[1].approverPublicKey = signed.publicKey;
  await f.queue.signedDecision('device-b', { requestId: pending.requestId, approve: true, remember: false, signature: signed.signature }, origin);
  assert.equal(f.unlocks(), 1);
});
