/* Apercu strictement factice, sans compte et sans sauvegarde des formulaires. */
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
const files = { '/': ['index.html', 'text/html'], '/app.js': ['app.js', 'text/javascript'], '/sharing-ui.js': ['sharing-ui.js', 'text/javascript'], '/style.css': ['style.css', 'text/css'] };
const agentId = '11111111-1111-4111-8111-111111111111';
createServer(async (request, response) => {
  const url = new URL(request.url, 'http://localhost');
  if (url.pathname === '/api/admin/state') { const id = url.searchParams.get('vaultId') || 'default'; response.setHeader('content-type', 'application/json'); response.end(JSON.stringify({ vaultId: id, vaultName: id === 'default' ? 'Admin factice' : 'Agent factice', vaults: [{ id: 'default', name: 'Admin factice' }, { id: agentId, name: 'Agent factice' }], configured: true, unlocked: true, devices: [], availableClients: [], account: { server: 'https://vault.example.test', email: 'probe@example.invalid', timeoutMinutes: 15 } })); return; }
  if (request.method === 'POST' && url.pathname === '/api/admin/sharing') { response.setHeader('content-type', 'application/json'); response.end(JSON.stringify({ organizations: [{ id: agentId, name: 'Famille factice', collections: [{ id: '22222222-2222-4222-8222-222222222222', name: 'Commun', canEdit: true }] }] })); return; }
  if (request.method === 'POST' && url.pathname === '/api/admin/write-preview') { response.setHeader('content-type', 'application/json'); response.end(JSON.stringify({ items: [{ id: '33333333-3333-4333-8333-333333333333', type: 1, name: 'Exemple partagé', revisionDate: '2026-11-08T00:00:00.000Z', organizationId: agentId, collectionIds: ['22222222-2222-4222-8222-222222222222'] }], folders: [] })); return; }
  if (request.method !== 'GET' || !files[request.url]) { response.writeHead(400); response.end('{}'); return; }
  const [file, type] = files[request.url]; response.setHeader('content-type', type + '; charset=utf-8'); response.end(await readFile(resolve('linux/web', file)));
}).listen(0, '127.0.0.1', function () { console.log(JSON.stringify({ previewPort: this.address().port, fictitiousOnly: true })); });
