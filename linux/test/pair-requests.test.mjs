import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Store } from '../store.mjs';
import { Session } from '../session.mjs';
import { Devices } from '../devices.mjs';
import { PairRequests } from '../pair-requests.mjs';
import { createApp } from '../http.mjs';

const fictitious = 'Fictif-essai-42!';
async function fixture(t) {
  const directory = await mkdtemp(join(tmpdir(), 'vaultagent-pair-')); t.after(() => rm(directory, { recursive: true, force: true }));
  const store = await new Store(directory).init(); let now = 0;
  const factory = async () => ({ alive: true, kill() { this.alive = false; }, async request(request) { return request.op === 'prepare' ? { restartRequired: false } : { unlocked: true }; } });
  const session = new Session(store, factory, () => now); const devices = new Devices(store, () => now);
  await session.configure({ server: 'https://vault.example.test', email: 'probe@example.invalid', password: fictitious, timeoutMinutes: 15 });
  return { store, session, devices, context: { id: 'default', devices }, advance: milliseconds => now += milliseconds, clock: () => now };
}

test('demande approuvée : code de contrôle, jeton remis une seule fois, appareil créé', async t => {
  const f = await fixture(t); const requests = new PairRequests(f.clock);
  const opened = requests.open('  Agent\u0007 Linux  ');
  assert.match(opened.verification, /^[A-HJ-NP-Z2-9]{4}-[A-HJ-NP-Z2-9]{4}$/);
  assert.deepEqual(requests.poll(opened.requestId, opened.secret).state, 'pending');
  assert.equal(requests.list()[0].name, 'Agent Linux'); assert.equal(requests.list()[0].verification, opened.verification);
  assert.ok(!JSON.stringify(requests.list()).includes(opened.secret));
  await requests.approve(opened.requestId, f.context);
  const result = requests.poll(opened.requestId, opened.secret);
  assert.equal(result.state, 'approved'); assert.equal(result.vaultId, 'default'); assert.equal(f.devices.authenticate('Bearer ' + result.token), result.deviceId);
  assert.equal(f.devices.list()[0].name, 'Agent Linux');
  assert.throws(() => requests.poll(opened.requestId, opened.secret), /pairing-refused/);
});

test('secret erroné, refus, expiration et limites', async t => {
  const f = await fixture(t); const requests = new PairRequests(f.clock);
  const first = requests.open('Premier');
  assert.throws(() => requests.poll(first.requestId, 'autre-secret'), /pairing-refused/);
  assert.throws(() => requests.poll('inconnu', first.secret), /pairing-refused/);
  requests.deny(first.requestId); assert.equal(requests.poll(first.requestId, first.secret).state, 'denied');
  assert.throws(() => requests.poll(first.requestId, first.secret), /pairing-refused/);
  const second = requests.open('Second'); f.advance(300001);
  await assert.rejects(requests.approve(second.requestId, f.context), /request-expired/);
  assert.throws(() => requests.poll(second.requestId, second.secret), /pairing-refused/);
  assert.throws(() => requests.open(''), /invalid-device/); assert.throws(() => requests.open('x'.repeat(81)), /invalid-device/);
  for (let index = 0; index < 8; index += 1) requests.open('Appareil ' + index);
  assert.throws(() => requests.open('Trop'), /pairing-limit/);
});

test('limite de débit : dix ouvertures par minute', async t => {
  const f = await fixture(t); const requests = new PairRequests(f.clock);
  for (let index = 0; index < 10; index += 1) { requests.open('Appareil ' + index); requests.entries.clear(); }
  assert.throws(() => requests.open('Onzième'), /pairing-rate-limit/);
  f.advance(60001); requests.open('Après la minute');
});

test('API : demande publique, approbation réservée au propriétaire, coffre non configuré refusé', async t => {
  const f = await fixture(t);
  const server = createApp({ session: f.session, devices: f.devices, origin: 'https://service.example.test', owner: 'Owner', trustedProxy: '127.0.0.1', webDirectory: tmpdir() });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); t.after(() => new Promise(resolve => server.close(resolve)));
  const base = 'http://127.0.0.1:' + server.address().port;
  const post = (path, input, headers = {}) => fetch(base + path, { method: 'POST', headers: { 'content-type': 'application/json', ...headers }, body: JSON.stringify(input) });
  const admin = { 'x-authentik-username': 'Owner', origin: 'https://service.example.test' };
  const opened = await (await post('/api/pair', { action: 'request', name: 'Serveur factice' })).json();
  assert.equal((await (await post('/api/pair', { action: 'poll', requestId: opened.requestId, secret: opened.secret })).json()).state, 'pending');
  assert.equal((await post('/api/admin/pair-decide', { requestId: opened.requestId, approve: true }, { 'x-authentik-username': 'Intrus', origin: 'https://service.example.test' })).status, 403);
  const state = await (await fetch(base + '/api/admin/state', { headers: { 'x-authentik-username': 'Owner' } })).json();
  assert.equal(state.pairRequests[0].verification, opened.verification);
  assert.equal((await post('/api/admin/pair-decide', { requestId: opened.requestId, approve: true }, admin)).status, 200);
  const result = await (await post('/api/pair', { action: 'poll', requestId: opened.requestId, secret: opened.secret })).json();
  assert.equal(result.state, 'approved'); assert.equal(f.devices.authenticate('Bearer ' + result.token), result.deviceId);
  assert.equal((await post('/api/pair', { action: 'poll', requestId: opened.requestId, secret: opened.secret })).status, 400);
});

const { generateKeyPairSync, sign, constants: cryptoConstants } = await import('node:crypto');
const { decisionMessage } = await import('../unlock-requests.mjs');
const rsa = () => { const pair = generateKeyPairSync('rsa', { modulusLength: 2048 }); return { publicKey: pair.publicKey.export({ type: 'spki', format: 'der' }).toString('base64'), sign: message => sign('sha256', Buffer.from(message), { key: pair.privateKey, padding: cryptoConstants.RSA_PKCS1_PSS_PADDING, saltLength: 32 }).toString('base64') }; };

test('association : la clé du PC approbateur est enregistrée à l\'approbation du propriétaire', async t => {
  const f = await fixture(t); const requests = new PairRequests(f.clock); const approver = rsa();
  assert.throws(() => requests.open('Mauvaise clé', 'pas-une-cle'), /invalid-key/);
  const opened = requests.open('PC approbateur', approver.publicKey);
  assert.equal(requests.list()[0].approver, true);
  await requests.approve(opened.requestId, f.context);
  const result = requests.poll(opened.requestId, opened.secret);
  assert.equal(f.devices.record(result.deviceId).approverPublicKey, approver.publicKey);
  const plain = requests.open('Client sans approbateur'); await requests.approve(plain.requestId, f.context);
  assert.equal(f.devices.record(requests.poll(plain.requestId, plain.secret).deviceId).approverPublicKey, undefined);
});

test('élévation : enrôlement signé par les deux clés, puis approbation exigeant la signature élevée', async t => {
  const f = await fixture(t); const { UnlockRequests } = await import('../unlock-requests.mjs'); const origin = 'https://vaultagent.example.test';
  const approver = rsa(); const elevation = rsa(); const impostor = rsa();
  const requests = new PairRequests(f.clock); const opened = requests.open('TowerZ', approver.publicKey); await requests.approve(opened.requestId, f.context);
  const device = requests.poll(opened.requestId, opened.secret); const queue = new UnlockRequests(f.session, f.devices, f.clock);
  const begin = queue.beginElevation(device.deviceId); const enrollMessage = decisionMessage(origin, 'enroll-elevation', begin.challenge, true, false);
  await assert.rejects(queue.enrollElevation(device.deviceId, { publicKey: elevation.publicKey, signature: approver.sign(enrollMessage), elevationSignature: impostor.sign(enrollMessage) }, origin), /elevation-required/);
  await assert.rejects(queue.enrollElevation(device.deviceId, { publicKey: elevation.publicKey, signature: impostor.sign(enrollMessage), elevationSignature: elevation.sign(enrollMessage) }, origin), /approval-refused/);
  const second = queue.beginElevation(device.deviceId); const message = decisionMessage(origin, 'enroll-elevation', second.challenge, true, false);
  assert.deepEqual(await queue.enrollElevation(device.deviceId, { publicKey: elevation.publicKey, signature: approver.sign(message), elevationSignature: elevation.sign(message) }, origin), { elevationEnabled: true, vaultId: 'default' });
  assert.throws(() => queue.beginElevation(device.deviceId), /elevation-enrollment-refused/);
  assert.equal((await queue.security(device.deviceId)).elevationEnabled, true);
  f.session.lock(); const pending = await queue.request(device.deviceId); const entry = queue.list()[0];
  const decide = (approve, signer) => { const text = decisionMessage(origin, entry.requestId, entry.challenge, approve, false); return { requestId: entry.requestId, approve, remember: false, signature: approver.sign(text), elevationSignature: signer?.sign(text) }; };
  assert.equal(pending.state, 'pending');
  await assert.rejects(queue.signedDecision(device.deviceId, decide(true), origin), /elevation-required/);
  await assert.rejects(queue.signedDecision(device.deviceId, decide(true, impostor), origin), /elevation-required/);
  const refused = await queue.signedDecision(device.deviceId, decide(false), origin); assert.equal(refused.state, 'denied');
  f.advance(300001); f.session.lock(); await queue.request(device.deviceId);
  const next = queue.list()[0]; const text = decisionMessage(origin, next.requestId, next.challenge, true, false);
  const approved = await queue.signedDecision(device.deviceId, { requestId: next.requestId, approve: true, remember: false, signature: approver.sign(text), elevationSignature: elevation.sign(text) }, origin);
  assert.equal(approved.state, 'approved'); assert.equal(f.session.status().unlocked, true);
});
