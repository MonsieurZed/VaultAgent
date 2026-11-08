import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { generateKeyPairSync, privateDecrypt, createDecipheriv, constants } from 'node:crypto';
import { Store } from '../store.mjs';
import { Session } from '../session.mjs';
import { Devices } from '../devices.mjs';
import { createApp } from '../http.mjs';

const itemId = '00000000-0000-4000-8000-000000000001';
const fictitious = 'Fictif-essai-42!';
const item = { id: itemId, type: 1, name: 'Compte factice', login: { username: 'probe@example.invalid', password: fictitious, totp: 'fictitious-not-a-real-seed', uris: [{ uri: 'https://example.test/login' }] } };
async function fixture(t) {
  const directory = await mkdtemp(join(tmpdir(), 'vaultagent-test-')); t.after(() => rm(directory, { recursive: true, force: true }));
  const store = await new Store(directory).init(); let now = 0; let delay;
  const engines = [];
  const factory = async () => {
    const engine = { alive: true, kill() { this.alive = false; }, async request(request) {
      if (request.op === 'prepare') return { restartRequired: false };
      if (request.op === 'connect') { assert.equal(request.password, fictitious); return { unlocked: true }; }
      if (request.path.startsWith('/list/')) return { data: [structuredClone(item)] };
      if (request.path.startsWith('/object/item/')) { if (delay) await delay; return structuredClone(item); }
      if (request.path.startsWith('/object/totp/')) return { data: '123456' };
      throw new Error('mock-route');
    } }; engines.push(engine); return engine;
  };
  const session = new Session(store, factory, () => now); const devices = new Devices(store, () => now);
  const configure = (timeoutMinutes = 15) => session.configure({ server: 'https://vault.example.test', email: 'probe@example.invalid', password: fictitious, timeoutMinutes });
  return { store, session, devices, configure, engines, advance: milliseconds => now += milliseconds, delay: value => delay = value };
}
test('le mot de passe est chiffre sur disque ; le statut et le catalogue ne le contiennent pas', async t => {
  const f = await fixture(t); await f.configure();
  assert.ok(!(await readFile(join(f.store.directory, 'account.json'), 'utf8')).includes(fictitious));
  assert.equal(f.store.unprotect(f.store.account.password), fictitious);
  assert.ok(!JSON.stringify(f.session.status()).includes(fictitious));
  const catalog = await f.session.catalog(); assert.equal(catalog[0].password, true); assert.deepEqual(catalog[0].origins, ['https://example.test']); assert.ok(!JSON.stringify(catalog).includes(fictitious));
  f.session.lock(); assert.equal(f.engines[0].alive, false); await assert.rejects(f.session.catalog(), /locked/);
});
test('le delai glisse, status ne le prolonge pas, le delai peut etre desactive', async t => {
  const f = await fixture(t); await f.configure(1); f.advance(50000); f.session.status(); f.advance(11000); assert.equal(f.session.status().unlocked, false);
  await f.session.unlock(); f.advance(50000); await f.session.catalog(); f.advance(50000); assert.equal(f.session.status().unlocked, true); f.advance(11000); assert.equal(f.session.status().unlocked, false);
  await f.configure(null); f.advance(1000000000); assert.equal(f.session.status().unlocked, true); f.session.lock(); assert.equal(f.session.status().unlocked, false);
});
test('enveloppe interoperable RSA-OAEP/AES-GCM ; origine et champ verifies', async t => {
  const f = await fixture(t); await f.configure(); const { publicKey, privateKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const input = { itemId, field: 'password', origin: 'https://example.test', publicKey: publicKey.export({ type: 'spki', format: 'der' }).toString('base64') };
  const envelope = await f.session.secret(input); assert.ok(!JSON.stringify(envelope).includes(fictitious));
  const key = privateDecrypt({ key: privateKey, oaepHash: 'sha256', padding: constants.RSA_PKCS1_OAEP_PADDING }, Buffer.from(envelope.encryptedKey, 'base64'));
  const decoder = createDecipheriv('aes-256-gcm', key, Buffer.from(envelope.nonce, 'base64')); decoder.setAuthTag(Buffer.from(envelope.tag, 'base64'));
  const clear = Buffer.concat([decoder.update(Buffer.from(envelope.data, 'base64')), decoder.final()]); assert.equal(clear.toString(), fictitious); clear.fill(0); key.fill(0);
  await assert.rejects(f.session.secret({ ...input, origin: 'https://other.test' }), /origin-refused/);
  await assert.rejects(f.session.secret({ ...input, field: 'item' }), /invalid-field/);
  f.session.lock();
});
test('un verrouillage puis deverrouillage invalide une lecture en cours', async t => {
  const f = await fixture(t); await f.configure(); let release; f.delay(new Promise(resolve => release = resolve));
  const { publicKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const pending = f.session.secret({ itemId, field: 'password', origin: 'https://example.test', publicKey: publicKey.export({ type: 'spki', format: 'der' }).toString('base64') });
  await new Promise(resolve => setImmediate(resolve)); f.session.lock(); release(); await assert.rejects(pending, /locked/); await f.session.unlock(); assert.equal(f.session.status().unlocked, true); f.session.lock();
});
test('association a usage unique, expiration, stockage hache et revocation', async t => {
  const f = await fixture(t); const pairing = f.devices.issue('Client factice'); const result = await f.devices.pair(pairing.code);
  assert.equal(f.devices.authenticate('Bearer ' + result.token), result.deviceId);
  assert.ok(!(await readFile(join(f.store.directory, 'devices.json'), 'utf8')).includes(result.token));
  await assert.rejects(f.devices.pair(pairing.code), /pairing-refused/);
  const expired = f.devices.issue('Expire'); f.advance(300001); await assert.rejects(f.devices.pair(expired.code), /pairing-refused/);
  await f.devices.revoke(result.deviceId); assert.throws(() => f.devices.authenticate('Bearer ' + result.token), /unauthorized/);
});
test('API : proxy, proprietaire, CSRF, privileges clients et absence de lecture brute', async t => {
  const f = await fixture(t); await f.configure(); const owner = 'OwnerTest'; const origin = 'https://agent.example.test';
  const app = createApp({ session: f.session, devices: f.devices, origin, owner, trustedProxy: '127.0.0.1', webDirectory: resolve('linux/web') });
  await new Promise(resolve => app.listen(0, '127.0.0.1', resolve)); t.after(() => new Promise(resolve => app.close(resolve))); const base = 'http://127.0.0.1:' + app.address().port;
  const call = (path, data, headers = {}) => fetch(base + path, { method: data === undefined ? 'GET' : 'POST', headers: { 'content-type': 'application/json', ...headers }, ...(data === undefined ? {} : { body: JSON.stringify(data) }) });
  assert.equal((await call('/api/admin/state')).status, 403);
  assert.equal((await call('/api/admin/lock', {}, { 'x-authentik-username': owner })).status, 403);
  assert.equal((await call('/api/admin/state', undefined, { 'x-authentik-username': owner })).status, 200);
  const pairing = f.devices.issue('Agent factice'); const { token } = await f.devices.pair(pairing.code); const auth = { authorization: 'Bearer ' + token };
  assert.equal((await call('/api/agent/status', {}, auth)).status, 200);
  assert.equal((await call('/api/agent/unlock', {}, auth)).status, 400);
  assert.equal((await call('/api/agent/get-password', {}, auth)).status, 400);
  assert.equal((await call('/api/admin/unlock', {}, auth)).status, 403);
  assert.equal((await call('/api/agent/status', {}, { ...auth, origin: 'https://evil.test' })).status, 403);
  assert.equal((await call('/api/admin/lock', {}, { 'x-authentik-username': owner, origin })).status, 200);
  assert.equal((await call('/api/agent/catalog', {}, auth)).status, 423);
});
test('changement de compte : anciens jetons et codes d association refuses', async t => {
  const f = await fixture(t); await f.configure(); const pending = f.devices.issue('Code avant changement');
  const old = await f.devices.pair(f.devices.issue('Ancien client').code);
  f.store.account = { ...f.store.account, profile: 'different-profile' };
  assert.throws(() => f.devices.authenticate('Bearer ' + old.token), /unauthorized/);
  await assert.rejects(f.devices.pair(pending.code), /pairing-refused/); assert.deepEqual(f.devices.list(), []);
});
test('revocation pendant une lecture : aucune enveloppe retournee', async t => {
  const f = await fixture(t); await f.configure(); const client = await f.devices.pair(f.devices.issue('Client').code); let resume;
  f.delay(new Promise(resolve => resume = resolve)); const { publicKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const pending = f.session.secret({ itemId, field: 'password', origin: 'https://example.test', publicKey: publicKey.export({ type: 'spki', format: 'der' }).toString('base64') }, () => f.devices.authenticate('Bearer ' + client.token));
  await new Promise(resolve => setImmediate(resolve)); await f.devices.revoke(client.deviceId); resume(); await assert.rejects(pending, /unauthorized/); f.session.lock();
});
