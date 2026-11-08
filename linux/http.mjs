/* Le backend n'est joignable que depuis son proxy TLS prive. Authentik protege les routes admin. */
import { createServer } from 'node:http';
import { createReadStream } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { UnlockRequests } from './unlock-requests.mjs';

export function createApp({ session, devices, origin, owner, trustedProxy, webDirectory, downloadsDirectory }) {
  const approvals = new UnlockRequests(session, devices);
  const assets = { '/': ['index.html', 'text/html; charset=utf-8'], '/app.js': ['app.js', 'text/javascript; charset=utf-8'], '/style.css': ['style.css', 'text/css; charset=utf-8'] };
  const send = (response, code, value) => { response.writeHead(code, { 'content-type': 'application/json', 'cache-control': 'no-store', 'x-content-type-options': 'nosniff' }); response.end(JSON.stringify(value)); };
  async function body(request) {
    if (!/^application\/json(?:;|$)/i.test(request.headers['content-type'] || '')) throw new Error('json-required');
    let length = 0; const parts = [];
    for await (const part of request) { length += part.length; if (length > 24576) throw new Error('request-size'); parts.push(part); }
    return JSON.parse(Buffer.concat(parts).toString('utf8'));
  }
  const server = createServer(async (request, response) => {
    try {
      const path = new URL(request.url, origin).pathname;
      if (request.method === 'GET' && path === '/healthz') { send(response, 200, { alive: true }); return; }
      const remote = request.socket.remoteAddress.replace(/^::ffff:/, '');
      if (remote !== trustedProxy) throw new Error('proxy-required');
      if (request.headers.origin && request.headers.origin !== origin) throw new Error('origin-refused');
      if (request.method === 'POST' && path === '/api/pair') { send(response, 200, await devices.pair((await body(request)).code)); return; }
      if (path.startsWith('/api/agent/')) {
        const authorized = () => devices.authenticate(request.headers.authorization); authorized();
        if (request.method !== 'POST') throw new Error('method-refused'); const data = await body(request); authorized();
        if (path === '/api/agent/status') send(response, 200, { ...session.status(), unlockRequest: approvals.current(authorized()), approvalRequests: devices.record(authorized())?.approverPublicKey ? approvals.list() : [] });
        else if (path === '/api/agent/request-unlock') send(response, 200, await approvals.request(authorized()));
        else if (path === '/api/agent/decide-unlock') send(response, 200, await approvals.signedDecision(authorized(), data, origin));
        else if (path === '/api/agent/approval-context') send(response, 200, approvals.context(authorized()));
        else if (path === '/api/agent/catalog') send(response, 200, await session.catalog(data.search, authorized));
        else if (path === '/api/agent/seal') send(response, 200, await session.secret(data, authorized));
        else if (path === '/api/agent/write-preview') send(response, 200, await session.writePreview(data, authorized));
        else if (path === '/api/agent/write') send(response, 200, await session.write(data, authorized));
        else if (path === '/api/agent/lock') { session.lock(); send(response, 200, { locked: true }); }
        else throw new Error('route-refused'); return;
      }
      if (!owner || request.headers['x-authentik-username'] !== owner) throw new Error('owner-required');
      const download = { '/downloads/windows-client.zip': 'VaultAgent-Windows-Client-3.0.0-preview.3-x64.zip', '/downloads/linux-client.tar.gz': 'VaultAgent-Linux-Client-3.0.0-preview.3.tar.gz', '/downloads/windows-client.zip.sha256': 'VaultAgent-Windows-Client-3.0.0-preview.3-x64.zip.sha256', '/downloads/linux-client.tar.gz.sha256': 'VaultAgent-Linux-Client-3.0.0-preview.3.tar.gz.sha256' }[path];
      if (request.method === 'GET' && download && downloadsDirectory) {
        const stream = createReadStream(join(downloadsDirectory, download));
        response.once('close', () => stream.destroy());
        stream.once('error', () => { if (!response.headersSent) send(response, 404, { error: 'download-unavailable' }); else response.destroy(); });
        stream.once('open', () => { response.writeHead(200, { 'content-type': 'application/octet-stream', 'content-disposition': 'attachment; filename="' + download + '"', 'cache-control': 'no-store', 'x-content-type-options': 'nosniff' }); stream.pipe(response); }); return;
      }
      if (request.method === 'GET' && assets[path]) {
        const [file, type] = assets[path]; const content = await readFile(join(webDirectory, file));
        response.writeHead(200, { 'content-type': type, 'cache-control': 'no-store', 'content-security-policy': "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'", 'x-content-type-options': 'nosniff', 'referrer-policy': 'no-referrer' }); response.end(content); return;
      }
      if (request.method === 'GET' && path === '/api/admin/state') {
        const account = session.store.account;
        send(response, 200, { ...session.status(), account: account ? { server: account.server, email: account.email, timeoutMinutes: account.timeoutMinutes } : null, devices: devices.list(), unlockRequests: approvals.list() }); return;
      }
      if (request.method !== 'POST' || request.headers.origin !== origin) throw new Error('origin-refused');
      const data = await body(request);
      if (path === '/api/admin/configure') send(response, 200, await session.configure(data));
      else if (path === '/api/admin/unlock') send(response, 200, await session.unlock(data.code));
      else if (path === '/api/admin/decide-unlock') send(response, 200, await approvals.decide(data.requestId, data.approve, data.code));
      else if (path === '/api/admin/auto-unlock') { await devices.setAutomatic(data.deviceId, data.enabled); send(response, 200, { saved: true }); }
      else if (path === '/api/admin/lock') { session.lock(); send(response, 200, { locked: true }); }
      else if (path === '/api/admin/pairing') {
        if (!session.store.account) throw new Error('not-configured'); send(response, 200, devices.issue(data.name));
      } else if (path === '/api/admin/revoke') { session.lock(); await devices.revoke(data.deviceId); send(response, 200, { revoked: true }); }
      else throw new Error('route-refused');
    } catch (error) {
      const code = /^[a-z-]+(?::[a-z-]+)?$/.test(error.message || '') ? error.message : 'request-refused';
      const status = ['unauthorized', 'owner-required', 'proxy-required', 'origin-refused'].includes(code) ? 403 : code === 'locked' ? 423 : 400;
      if (!response.headersSent) send(response, status, { error: code }); else response.end();
    }
  });
  server.requestTimeout = 120000; server.headersTimeout = 10000; server.maxHeadersCount = 40; return server;
}
