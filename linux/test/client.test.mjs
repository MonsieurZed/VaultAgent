import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:https';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { generateKeyPairSync } from 'node:crypto';
import { seal } from '../session.mjs';

test('helper distant : HTTPS verifie, remplissage factice et retour sans valeur secrete', async t => {
  const directory = await mkdtemp(join(tmpdir(), 'vaultagent-client-test-')); t.after(() => rm(directory, { recursive: true, force: true }));
  const cert = process.platform === 'win32' ? spawnSync('pwsh.exe', ['-NoProfile', '-File', resolve('tests/create-test-certificate.ps1'), '-Directory', directory], { windowsHide: true }) :
    spawnSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', join(directory, 'test.key'), '-out', join(directory, 'test.crt'), '-days', '1', '-subj', '/CN=127.0.0.1', '-addext', 'subjectAltName=IP:127.0.0.1', '-addext', 'basicConstraints=critical,CA:TRUE']);
  assert.equal(cert.status, 0);
  const password = 'VaultAgent-Remote-Fictif-42!'; const fakeToken = 'TEST-TOKEN-WITHOUT-ACCESS';
  const server = createServer({ key: await readFile(join(directory, 'test.key')), cert: await readFile(join(directory, 'test.crt')) }, async (request, response) => {
    assert.equal(request.headers.authorization, 'Bearer ' + fakeToken); let body = ''; for await (const part of request) body += part;
    const data = JSON.parse(body);
    if (request.url === '/api/agent/request-unlock') { response.end(JSON.stringify({ requestId: 'test-request', state: 'pending' })); return; }
    if (request.url === '/api/agent/status') { response.end(JSON.stringify({ unlocked: false, unlockRequest: { requestId: 'test-request', state: 'denied' } })); return; }
    if (request.url === '/api/agent/write') {
      assert.equal(data.action, 'create'); assert.equal(data.item.login.password, password); assert.equal(data.item.name, 'Exemple factice');
      response.setHeader('content-type', 'application/json'); response.end(JSON.stringify({ verified: true, id: '00000000-0000-4000-8000-000000000001' })); return;
    }
    assert.equal(request.url, '/api/agent/seal'); assert.equal(data.origin, 'https://example.test');
    response.setHeader('content-type', 'application/json'); response.end(JSON.stringify(seal(password, data.publicKey)));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); t.after(() => new Promise(resolve => server.close(resolve)));
  const file = join(directory, 'client.json'); await writeFile(file, JSON.stringify({ server: 'https://127.0.0.1:' + server.address().port, token: fakeToken }), { mode: 0o600 });
  const script = `import {fillBrowser,saveFromSource,unlock} from ${JSON.stringify(new URL('../../remote-client/agent-tools.mjs', import.meta.url).href)};
const approval=await unlock({waitSeconds:5,configFile:${JSON.stringify(file)}});if(approval.state!=='denied'||approval.unlocked!==false)throw Error('refusal-not-respected');
let filled=false;const result=await fillBrowser({locator:{fill:async value=>{if(value!==${JSON.stringify(password)})throw new Error('fake-fill-mismatch');filled=true;}},itemId:'00000000-0000-4000-8000-000000000001',field:'password',origin:'https://example.test',configFile:${JSON.stringify(file)}});
if(!filled)throw new Error('not-filled');
const saved=await saveFromSource({operationId:'00000000-0000-4000-8000-000000000002',action:'create',metadata:{type:1,name:'Exemple factice'},source:async()=>({password:${JSON.stringify(password)}}),configFile:${JSON.stringify(file)}});
if(!saved.verified)throw new Error('not-saved');console.log(JSON.stringify(result));`;
  const output = await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ['--input-type=module', '-e', script], { windowsHide: true, env: { ...process.env, NODE_EXTRA_CA_CERTS: join(directory, 'test.crt') } }); let text = ''; let errors = '';
    child.stdout.on('data', part => text += part); child.stderr.on('data', part => errors += part); child.once('error', reject); child.once('exit', code => code === 0 ? resolve(text) : reject(new Error('Test helper failed: ' + errors)));
  });
  assert.deepEqual(JSON.parse(output), { filled: true, field: 'password', clipboardUsed: false }); assert.ok(!output.includes(password)); assert.ok(!output.includes(fakeToken));
  const { privateKey, publicKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const fixture = join(directory, 'envelope.json'); await writeFile(fixture, JSON.stringify({ privateKey: privateKey.export({ type: 'pkcs8', format: 'der' }).toString('base64'), envelope: seal(password, publicKey.export({ type: 'spki', format: 'der' }).toString('base64')) }));
  if (process.platform === 'win32') {
    const native = spawnSync('pwsh.exe', ['-NoProfile', '-File', resolve('tests/remote-crypto.ps1'), '-Fixture', fixture], { windowsHide: true, encoding: 'utf8' });
    assert.equal(native.status, 0, native.stderr); assert.match(native.stdout, /NODE_DOTNET_ENVELOPE_OK/);
  }
});
