/* Banc local de collage : une valeur de test publique, aucune donnée du coffre. */
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '../runtime/tests');
fs.mkdirSync(root, { recursive: true });
const expected = 'vault-agent-paste-probe-20261107';
const page = `<!doctype html><meta charset="utf-8"><title>Test local de collage</title>
<style>body{font:18px system-ui;max-width:620px;margin:70px auto;background:#faf8ff;color:#302742}input,button{display:block;font:inherit;margin:15px 0;padding:12px}button{background:#7948d9;color:white;border:0;border-radius:8px}</style>
<h1>Test local de collage sans lecture</h1><p>Valeur factice. Aucun identifiant réel.</p>
<label for="password">Champ masqué de test</label><input id="password" type="password" autocomplete="off">
<button id="verify">Vérifier le collage</button><p id="result" role="status">En attente</p>
<script>document.querySelector('#verify').onclick=async()=>{const r=await fetch('/verify',{method:'POST',body:document.querySelector('#password').value});const j=await r.json();document.querySelector('#result').textContent=j.matched?'Collage validé sans lecture de la valeur':'Aucune correspondance';};</script>`;
const server = http.createServer((request, response) => {
  if (request.url === '/features') {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(`<!doctype html><meta charset="utf-8"><title>Capacités WebAuthn</title><h1>Capacités passkeys du navigateur intégré</h1><pre id="result">Vérification…</pre><script>(async()=>{const value={secureContext:isSecureContext,webauthn:typeof PublicKeyCredential!=='undefined'};if(value.webauthn){try{value.platformAuthenticator=await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()}catch{value.platformAuthenticator=false}}document.querySelector('#result').textContent=JSON.stringify(value,null,2)})()</script>`);
    return;
  }
  if (request.url === '/verify' && request.method === 'POST') {
    let value = ''; request.on('data', chunk => { value += chunk; if (value.length > 2048) request.destroy(); });
    request.on('end', () => {
      const result = { matched: value === expected, testedUtc: new Date().toISOString() };
      fs.writeFileSync(path.join(root, 'paste-result.json'), JSON.stringify(result, null, 2));
      response.writeHead(200, { 'content-type': 'application/json' }); response.end(JSON.stringify(result));
    });
  } else { response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); response.end(page); }
});
server.listen(Number(process.argv[2]) || 0, '127.0.0.1', () => {
  const value = { url: 'http://127.0.0.1:' + server.address().port + '/' };
  fs.writeFileSync(path.join(root, 'paste-server.json'), JSON.stringify(value));
  process.stdout.write(JSON.stringify(value) + '\n');
});
