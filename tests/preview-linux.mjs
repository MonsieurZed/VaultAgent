/* Apercu strictement factice, sans compte et sans sauvegarde des formulaires. */
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
const files = { '/': ['index.html', 'text/html'], '/app.js': ['app.js', 'text/javascript'], '/style.css': ['style.css', 'text/css'] };
createServer(async (request, response) => {
  if (request.url === '/api/admin/state') { response.setHeader('content-type', 'application/json'); response.end(JSON.stringify({ configured: false, unlocked: false, devices: [], account: null })); return; }
  if (request.method !== 'GET' || !files[request.url]) { response.writeHead(400); response.end('{}'); return; }
  const [file, type] = files[request.url]; response.setHeader('content-type', type + '; charset=utf-8'); response.end(await readFile(resolve('linux/web', file)));
}).listen(0, '127.0.0.1', function () { console.log(JSON.stringify({ previewPort: this.address().port, fictitiousOnly: true })); });
