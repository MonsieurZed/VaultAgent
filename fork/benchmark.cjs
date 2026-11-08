/* But : mesurer les démarrages et les appels chauds sans ouvrir un coffre ni lire de secret. */
const { spawn, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const readline = require('node:readline');
const { performance } = require('node:perf_hooks');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'vault-benchmark-'));
const engine = path.resolve(__dirname, '../runtime/engine/bw.js');
const original = process.argv[2];
const environment = { ...process.env, BITWARDENCLI_APPDATA_DIR: path.join(root, 'profile'), BW_NOINTERACTION: 'true',
  NODE_PATH: path.resolve(__dirname, '../.build-deps/node_modules') };
delete environment.BW_SESSION;
let child;
async function main() {
  const cold = [];
  for (let i = 0; i < 3; i++) {
    const start = performance.now();
    const result = spawnSync(process.execPath, [original, 'status'], { env: environment, windowsHide: true, timeout: 60000 });
    if (result.status !== 0) throw new Error('baseline failed');
    cold.push(performance.now() - start);
  }
  const start = performance.now();
  child = spawn(process.execPath, [engine], { env: environment, windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'] });
  const lines = readline.createInterface({ input: child.stdout, crlfDelay: Infinity })[Symbol.asyncIterator]();
  const ready = JSON.parse((await lines.next()).value);
  if (!ready.ready) throw new Error('engine failed');
  const startup = performance.now() - start;
  const warm = [];
  for (let i = 0; i < 20; i++) {
    const time = performance.now();
    child.stdin.write(JSON.stringify({ id: i, op: 'call', method: 'GET', path: '/status' }) + '\n');
    const response = JSON.parse((await lines.next()).value);
    if (!response.success || response.id !== i) throw new Error('request failed');
    warm.push(performance.now() - time);
  }
  const mean = values => values.reduce((a, b) => a + b, 0) / values.length;
  const result = { measuredUtc: new Date().toISOString(), operation: 'status, empty isolated profile',
    baselineMeanMs: Math.round(mean(cold)), engineStartupMs: Math.round(startup), warmMeanMs: +mean(warm).toFixed(2),
    warmP95Ms: +warm.sort((a, b) => a - b)[18].toFixed(2), baselineSamples: cold.length, warmSamples: warm.length };
  fs.writeFileSync(path.resolve(__dirname, '../benchmark.json'), JSON.stringify(result, null, 2));
  process.stdout.write(JSON.stringify(result, null, 2) + '\n');
}
main().catch(error => { process.stderr.write(error.message + '\n'); process.exitCode = 1; }).finally(() => {
  if (child) child.kill();
  fs.rmSync(root, { recursive: true, force: true });
});
