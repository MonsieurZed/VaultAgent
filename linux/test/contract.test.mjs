/* Rejeu des cas dorés de rust/contrat contre le serveur Node (tâche 0.3). Fichier provisoire, supprimé à la tâche 5.2. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm } from 'node:fs/promises';
import { readdirSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { generateKeyPairSync, randomUUID } from 'node:crypto';
import { Store } from '../store.mjs';
import { Session } from '../session.mjs';
import { Devices } from '../devices.mjs';
import { createApp } from '../http.mjs';
import { applyWritePatch } from '../../fork/write-patch.mjs';

const CASES = fileURLToPath(new URL('../../rust/contrat/', import.meta.url));
const WEB = fileURLToPath(new URL('../web', import.meta.url));
const PROTOCOL = fileURLToPath(new URL('../../docs/rust/protocole.md', import.meta.url));
const SENTINEL = 'Fictif-essai-42!';
const ITEM_ID = '00000000-0000-4000-8000-000000000001';
const FOLDER_ID = '00000000-0000-4000-8000-000000000005';
const OPERATION_ID = '00000000-0000-4000-8000-0000000000a1';
const REVISION = '2026-11-08T00:00:00.000Z';
const UPDATED = '2026-11-08T00:01:00.000Z';
const ORIGIN = 'https://service.example.test';
const OWNER = 'Owner';
const KEYS = generateKeyPairSync('rsa', { modulusLength: 2048 });
const SPKI = KEYS.publicKey.export({ type: 'spki', format: 'der' }).toString('base64');
const ITEM = { id: ITEM_ID, type: 1, name: 'Compte factice', revisionDate: REVISION, login: { username: 'probe@example.invalid', password: SENTINEL, totp: 'fictitious-not-a-real-seed', uris: [{ uri: 'https://example.test/login' }] } };
const TOP_KEYS = ['version', 'name', 'description', 'fixture', 'request', 'response', 'steps'];
const REQUEST_KEYS = ['method', 'path', 'headers', 'body', 'rawBody', 'bodyPadding'];
const FIXTURE_VALUES = { vault: ['unconfigured', 'locked', 'unlocked'], client: ['none', 'paired', 'approver'], proxy: ['trusted', 'untrusted'], downloads: [false, true] };
const GENERIC = { uuid: '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}', 'iso-date': '\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}(?:\\.\\d+)?Z', base64: '[A-Za-z0-9+/]+={0,2}', string: '.+', number: '-?\\d+(?:\\.\\d+)?' };
const GENERIC_SPLIT = /(\{\{(?:uuid|iso-date|base64|string|number)\}\})/;
const caseFiles = readdirSync(CASES).filter(file => file.endsWith('.json')).sort();

/**
 * Crée le faux moteur à état : fiche factice, dossiers, création, mise à jour et corbeille.
 * @param {{items: object[], folders: object[]}} state État partagé du coffre factice.
 * @returns {object} Faux moteur compatible avec Session.
 */
function createEngine(state) {
  return { alive: true, kill() { this.alive = false; }, async request(request) {
    if (request.op === 'prepare') return { restartRequired: false };
    if (request.op === 'connect') { assert.equal(request.password, SENTINEL); return { unlocked: true }; }
    if (request.op === 'safe-update') {
      const item = state.items.find(value => value.id === request.itemId);
      applyWritePatch(item, request.expectedRevisionDate, request.patch); item.revisionDate = UPDATED; return { id: item.id };
    }
    return route(state, request);
  } };
}

/**
 * Répond aux appels REST du faux moteur.
 * @param {{items: object[], folders: object[]}} state État partagé du coffre factice.
 * @param {{method: string, path: string, body?: object}} request Appel reçu.
 * @returns {object} Réponse du moteur.
 */
function route(state, { method, path, body }) {
  if (path === '/sync') return {};
  if (path.startsWith('/list/object/items')) return { data: state.items.filter(item => !!item.deletedDate === path.includes('trash=true')).map(item => structuredClone(item)) };
  if (path === '/list/object/folders') return { data: structuredClone(state.folders) };
  if (path.startsWith('/object/totp/')) return { data: '123456' };
  if (method === 'POST' && path === '/object/folder') { const folder = { id: randomUUID(), name: body.name }; state.folders.push(folder); return folder; }
  if (method === 'POST' && path === '/object/item') { const item = { ...structuredClone(body), id: randomUUID(), revisionDate: REVISION }; state.items.push(item); return structuredClone(item); }
  const item = state.items.find(value => path === '/object/item/' + value.id);
  if (item && method === 'GET') return structuredClone(item);
  if (item && method === 'DELETE') { item.deletedDate = UPDATED; item.revisionDate = UPDATED; return {}; }
  throw new Error('mock-route');
}

/**
 * Monte une fixture neuve : dossier temporaire, session, appareils et serveur sur 127.0.0.1.
 * @param {object} spec Section fixture du cas (vault, client, proxy, downloads).
 * @returns {Promise<{base: string, values: object, close: Function}>} Adresse, valeurs de gabarit et fermeture.
 */
async function mount(spec) {
  const directory = await mkdtemp(join(tmpdir(), 'vaultagent-contract-'));
  const store = await new Store(directory).init();
  const state = { items: [structuredClone(ITEM)], folders: [{ id: FOLDER_ID, name: 'Dossier factice' }] };
  const session = new Session(store, async () => createEngine(state)); const devices = new Devices(store);
  const vault = spec.vault ?? 'unlocked'; const client = spec.client ?? 'none'; let token = '';
  if (vault !== 'unconfigured') await session.configure({ server: 'https://vault.example.test', email: 'probe@example.invalid', password: SENTINEL, timeoutMinutes: 15 });
  if (client === 'paired') token = (await devices.pair(devices.issue('Client factice').code)).token;
  if (client === 'approver') token = (await devices.enrollNamed('Approbateur factice', SPKI)).token;
  if (vault === 'locked') session.lock();
  const downloadsDirectory = spec.downloads ? join(directory, 'downloads') : undefined;
  if (downloadsDirectory) await mkdir(downloadsDirectory);
  const server = createApp({ session, devices, origin: ORIGIN, owner: OWNER, trustedProxy: spec.proxy === 'untrusted' ? '192.0.2.1' : '127.0.0.1', webDirectory: WEB, downloadsDirectory });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const values = { token: 'Bearer ' + token, origin: ORIGIN, owner: OWNER, itemId: ITEM_ID, rsaPublicKey: SPKI, operationId: OPERATION_ID };
  const close = async () => { session.lock(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); await rm(directory, { recursive: true, force: true }); };
  return { base: 'http://127.0.0.1:' + server.address().port, values, close };
}

/**
 * Remplace les gabarits connus (valeurs de l'essai et {{stepN.clé}}) dans une valeur JSON.
 * @param {*} value Valeur à parcourir.
 * @param {object} values Valeurs de gabarit de la fixture.
 * @param {object[]} steps Corps des réponses déjà reçues.
 * @returns {*} Copie avec les gabarits remplacés.
 */
function fill(value, values, steps) {
  if (Array.isArray(value)) return value.map(entry => fill(entry, values, steps));
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, fill(entry, values, steps)]));
  if (typeof value !== 'string') return value;
  return value.replace(/\{\{(token|origin|owner|itemId|rsaPublicKey|operationId|step\d+\.[\w-]+)\}\}/g, (_, name) => {
    const [step, key] = name.split('.'); const found = key ? steps[Number(step.slice(4)) - 1]?.[key] : values[name];
    assert.equal(typeof found, 'string', 'gabarit sans valeur : ' + name); return found;
  });
}

/**
 * Compare une valeur reçue à une valeur attendue, par inclusion et avec les valeurs génériques.
 * @param {*} expected Valeur attendue.
 * @param {*} actual Valeur reçue.
 * @param {string} where Chemin affiché en cas d'échec.
 * @returns {void}
 */
function compare(expected, actual, where) {
  if (typeof expected === 'string' && GENERIC_SPLIT.test(expected)) {
    if (expected === '{{number}}') return assert.equal(typeof actual, 'number', where);
    const pattern = expected.split(GENERIC_SPLIT).map(part => GENERIC_SPLIT.test(part) ? GENERIC[part.slice(2, -2)] : part.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('');
    return assert.ok(typeof actual === 'string' && new RegExp('^' + pattern + '$', 's').test(actual), where + ' : ' + JSON.stringify(actual) + ' ne vérifie pas ' + expected);
  }
  if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual) && actual.length === expected.length, where + ' : tableau de longueur différente');
    return expected.forEach((entry, index) => compare(entry, actual[index], where + '[' + index + ']'));
  }
  if (expected && typeof expected === 'object') {
    assert.ok(actual && typeof actual === 'object', where + ' : objet attendu');
    for (const [key, entry] of Object.entries(expected)) { assert.ok(key in actual, where + '.' + key + ' manquant'); compare(entry, actual[key], where + '.' + key); }
    return undefined;
  }
  return assert.equal(actual, expected, where);
}

/**
 * Envoie une requête de cas doré avec fetch et lit la réponse.
 * @param {string} base Adresse du serveur d'essai.
 * @param {object} request Requête du cas, gabarits déjà remplacés.
 * @returns {Promise<{status: number, headers: Headers, text: string, body: *}>} Réponse lue.
 */
async function send(base, request) {
  let body;
  if ('rawBody' in request) body = request.rawBody;
  else if ('body' in request || 'bodyPadding' in request) body = JSON.stringify(request.bodyPadding ? { ...request.body, padding: 'a'.repeat(request.bodyPadding) } : request.body);
  const response = await fetch(base + request.path, { method: request.method, headers: request.headers, body });
  const text = await response.text(); let parsed = text;
  try { parsed = JSON.parse(text); } catch { parsed = text; }
  return { status: response.status, headers: response.headers, text, body: parsed };
}

/**
 * Vérifie le format d'un cas doré : version, nom, clés connues, fixture et forme des requêtes.
 * @param {object} spec Cas lu depuis le fichier.
 * @param {string} file Nom du fichier du cas.
 * @returns {void}
 */
function validateCase(spec, file) {
  assert.equal(spec.version, 1, 'version'); assert.equal(spec.name, basename(file, '.json'), 'nom');
  assert.ok(typeof spec.description === 'string' && spec.description, 'description');
  for (const key of Object.keys(spec)) assert.ok(TOP_KEYS.includes(key), 'clé inconnue : ' + key);
  assert.ok(spec.fixture && typeof spec.fixture === 'object', 'fixture');
  for (const [key, value] of Object.entries(spec.fixture)) assert.ok(FIXTURE_VALUES[key]?.includes(value), 'fixture invalide : ' + key);
  assert.ok(spec.steps ? !spec.request && !spec.response && spec.steps.length > 0 : spec.request && spec.response, 'request/response ou steps');
  for (const step of stepsOf(spec)) {
    for (const key of Object.keys(step.request)) assert.ok(REQUEST_KEYS.includes(key), 'clé de requête inconnue : ' + key);
    assert.ok(step.request.method && step.request.path && Number.isInteger(step.response.status), 'méthode, chemin et statut');
  }
}

/**
 * Donne la liste des étapes d'un cas, qu'il soit simple ou à plusieurs requêtes.
 * @param {object} spec Cas doré.
 * @returns {{request: object, response: object}[]} Étapes dans l'ordre.
 */
function stepsOf(spec) { return spec.steps ?? [{ request: spec.request, response: spec.response }]; }

/**
 * Rejoue un cas sur une fixture neuve et compare chaque réponse.
 * @param {object} spec Cas doré validé.
 * @returns {Promise<void>}
 */
async function replay(spec) {
  const fixture = await mount(spec.fixture); const received = [];
  try {
    for (const [index, step] of stepsOf(spec).entries()) {
      const request = fill(step.request, fixture.values, received); const expected = fill(step.response, fixture.values, received);
      const reply = await send(fixture.base, request); received.push(reply.body); const where = spec.name + '#' + (index + 1);
      assert.ok(!reply.text.includes(SENTINEL), where + ' : le mot de passe factice est sorti');
      assert.equal(reply.status, expected.status, where + ' : statut');
      for (const [name, value] of Object.entries(expected.headers ?? {})) compare(value, reply.headers.get(name), where + ' : en-tête ' + name);
      if ('body' in expected) compare(expected.body, reply.body, where + ' : corps');
    }
  } finally { await fixture.close(); }
}

for (const file of caseFiles) {
  test(basename(file, '.json'), async () => {
    const spec = JSON.parse(readFileSync(join(CASES, file), 'utf8')); validateCase(spec, file); await replay(spec);
  });
}

test('garde : au moins 30 cas et chaque route agent du protocole est couverte', () => {
  assert.ok(caseFiles.length >= 30, caseFiles.length + ' cas chargés');
  const section = readFileSync(PROTOCOL, 'utf8').split('## Routes agent')[1].split('**`status`**')[0];
  const routes = [...section.matchAll(/^\| `([a-z-]+)` \|/gm)].map(match => match[1]);
  assert.ok(routes.length >= 16, 'routes lues : ' + routes.length);
  const paths = new Set(caseFiles.flatMap(file => stepsOf(JSON.parse(readFileSync(join(CASES, file), 'utf8'))).map(step => step.request.path)));
  for (const name of routes) assert.ok(paths.has('/api/agent/' + name), 'route sans cas doré : ' + name);
});
