import { test } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, randomBytes, randomUUID, sign, createHash, constants } from 'node:crypto';
import { mkdtemp, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { verifyHello, helloOrigin, helloRpId, helloChallenge } from '../hello.mjs';
import { UnlockRequests, decisionMessage } from '../unlock-requests.mjs';
import { Store } from '../store.mjs';
import { Devices } from '../devices.mjs';

function authenticator(type = 'EC2') {
  const { publicKey, privateKey } = type === 'EC2' ? generateKeyPairSync('ec', { namedCurve: 'prime256v1' }) : generateKeyPairSync('rsa', { modulusLength: 2048 });
  const key = publicKey.export({ format: 'jwk' }); const b64 = value => Buffer.from(value, 'base64url').toString('base64');
  const credential = { id: randomBytes(32).toString('base64'), keyType: type, first: b64(key.x || key.n), second: b64(key.y || key.e) };
  return { credential, proof(message, overrides = {}) {
    const client = Buffer.from(JSON.stringify({ type: 'webauthn.get', challenge: helloChallenge(message), origin: helloOrigin, crossOrigin: false, ...overrides.client }));
    const auth = Buffer.alloc(37); createHash('sha256').update(overrides.rp || helloRpId).digest().copy(auth); auth[32] = overrides.flags ?? 5;
    const signed = Buffer.concat([auth, createHash('sha256').update(client).digest()]);
    const signature = sign('sha256', signed, type === 'RSA' ? { key: privateKey, padding: constants.RSA_PKCS1_PADDING } : { key: privateKey, dsaEncoding: 'der' });
    return { credentialId: credential.id, clientData: client.toString('base64'), authenticatorData: auth.toString('base64'), signature: signature.toString('base64') };
  } };
}
test('Hello EC2/RSA : challenge, origine, RP, presence, UV et signature sont verifies', () => {
  for (const type of ['EC2', 'RSA']) {
    const a = authenticator(type); const message = 'transaction-factice';
    assert.equal(verifyHello(a.credential, a.proof(message), message), true);
    for (const override of [{ flags: 1 }, { flags: 4 }, { rp: 'other.local' }, { client: { origin: 'https://other.local' } }, { client: { challenge: 'wrong' } }, { client: { type: 'webauthn.create' } }, { client: { crossOrigin: true } }]) assert.throws(() => verifyHello(a.credential, a.proof(message, override), message), /hello-invalid/);
    assert.throws(() => verifyHello(a.credential, a.proof(message), message + 'other'), /hello-invalid/);
    assert.throws(() => verifyHello(a.credential, { ...a.proof(message), signature: randomBytes(64).toString('base64') }, message), /hello-invalid/);
  }
});
test('preuve WebAuthn .NET factice validee par Node sans secret exporte', { skip: process.platform !== 'win32' }, () => {
  const message = decisionMessage('https://vaultagent.example.test', randomUUID(), randomBytes(32).toString('base64'), true, false);
  const result = spawnSync('pwsh.exe', ['-NoProfile', '-File', resolve('tests/hello-proof-crypto.ps1'), '-Message', message], { encoding: 'utf8', windowsHide: true });
  assert.equal(result.status, 0, result.stderr); const fixture = JSON.parse(result.stdout);
  assert.equal(verifyHello(fixture.credential, fixture.proof, message), true);
});
async function fixture(t) {
  const directory = await mkdtemp(join(tmpdir(), 'vaultagent-hello-test-')); t.after(() => rm(directory, { recursive: true, force: true }));
  const store = await new Store(directory).init(); store.account = { profile: 'fake' };
  const sourceId = randomUUID(); const approverId = randomUUID(); const otherId = randomUUID();
  const rsa = generateKeyPairSync('rsa', { modulusLength: 2048 });
  await store.updateDevices(() => [{ id: sourceId, profile: 'fake', name: 'Agent factice' }, ...[approverId, otherId].map(id => ({ id, profile: 'fake', name: 'PC factice', approverPublicKey: rsa.publicKey.export({ type: 'spki', format: 'der' }).toString('base64') }))]);
  let opened = false; let unlocks = 0; let now = 0;
  const session = { store, generation: 0, status: () => ({ unlocked: opened }), lock() { opened = false; ++this.generation; }, async unlock() { opened = true; ++unlocks; } };
  const devices = new Devices(store); const queue = new UnlockRequests(session, devices, () => now);
  const origin = 'https://vaultagent.example.test';
  const signed = (requestId, challenge, approve = true, remember = false) => {
    const message = decisionMessage(origin, requestId, challenge, approve, remember);
    return { message, signature: sign('sha256', Buffer.from(message), { key: rsa.privateKey, padding: constants.RSA_PKCS1_PSS_PADDING, saltLength: 32 }).toString('base64') };
  };
  return { store, sourceId, approverId, otherId, session, devices, queue, origin, signed, advance: ms => now += ms, unlocks: () => unlocks };
}
test('enrolement prouve les deux cles, persiste le mode Hello et interdit le remplacement client', async t => {
  const f = await fixture(t); const a = authenticator(); const context = await f.queue.beginHello(f.approverId);
  const s = f.signed(context.requestId, context.challenge);
  await assert.rejects(f.queue.enrollHello(f.approverId, { signature: s.signature, credential: a.credential }, f.origin), /hello-required/);
  await f.queue.enrollHello(f.approverId, { signature: s.signature, credential: a.credential, helloProof: a.proof(s.message) }, f.origin);
  assert.equal((await f.queue.security(f.approverId)).helloEnabled, true);
  assert.equal((await f.queue.security(f.otherId)).helloRequired, true);
  assert.equal((await f.store.load('approval-policy.json')).requireHello, true);
  await assert.rejects(f.queue.beginHello(f.otherId), /hello-enrollment-authorization-required/);
  const replacement = authenticator(); const next = await f.queue.beginHello(f.approverId); const nextSigned = f.signed(next.requestId, next.challenge);
  await assert.rejects(f.queue.enrollHello(f.approverId, { signature: nextSigned.signature, credential: replacement.credential, helloProof: replacement.proof(nextSigned.message) }, f.origin), /hello-enrollment-refused/);
  const request = await f.queue.request(f.sourceId); const challenge = f.queue.list()[0].challenge;
  const decision = f.signed(request.requestId, challenge);
  await assert.rejects(f.queue.signedDecision(f.approverId, { requestId: request.requestId, approve: true, remember: false, signature: decision.signature }, f.origin), /hello-required/);
  await assert.rejects(f.queue.signedDecision(f.otherId, { requestId: request.requestId, approve: true, remember: false, signature: decision.signature, helloProof: a.proof(decision.message) }, f.origin));
  assert.equal(f.unlocks(), 0);
  await f.queue.signedDecision(f.approverId, { requestId: request.requestId, approve: true, remember: false, signature: decision.signature, helloProof: a.proof(decision.message) }, f.origin);
  assert.equal(f.unlocks(), 1);
  await assert.rejects(f.queue.signedDecision(f.approverId, { requestId: request.requestId, approve: true, remember: false, signature: decision.signature, helloProof: a.proof(decision.message) }, f.origin), /request-expired/);
});
test('nonce expire, refus sans Hello et acceptation automatique explicitement conservee', async t => {
  const f = await fixture(t); const a = authenticator(); let context = await f.queue.beginHello(f.approverId); let signed = f.signed(context.requestId, context.challenge);
  f.advance(60001); await assert.rejects(f.queue.enrollHello(f.approverId, { signature: signed.signature, credential: a.credential, helloProof: a.proof(signed.message) }, f.origin), /hello-enrollment-refused/);
  context = await f.queue.beginHello(f.approverId); signed = f.signed(context.requestId, context.challenge);
  await f.queue.enrollHello(f.approverId, { signature: signed.signature, credential: a.credential, helloProof: a.proof(signed.message) }, f.origin);
  const request = await f.queue.request(f.sourceId); const nonce = f.queue.list()[0].challenge;
  const refusal = f.signed(request.requestId, nonce, false);
  await f.queue.signedDecision(f.approverId, { requestId: request.requestId, approve: false, remember: false, signature: refusal.signature }, f.origin);
  assert.equal(f.unlocks(), 0); f.session.lock(); await f.devices.setAutomatic(f.sourceId, true);
  assert.equal((await f.queue.request(f.sourceId)).unlocked, true); assert.equal(f.unlocks(), 1);
});
