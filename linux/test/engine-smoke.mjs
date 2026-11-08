/* Nouveau profil isole en /tmp ; aucun compte ou profil utilisateur n'est lu. */
import { Engine } from '../engine.mjs';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
const directory = await mkdtemp(join(tmpdir(), 'vaultagent-engine-test-'));
const engine = new Engine(directory, '/app/runtime/engine/bw.js');
try {
  await engine.start(randomUUID()); const status = await engine.request({ op: 'call', method: 'GET', path: '/status' });
  if (status.status !== 'unauthenticated') throw new Error('unexpected-test-state');
  console.log(JSON.stringify({ engineStarted: true, isolatedProfile: true, unauthenticated: true }));
} finally { engine.kill(); await rm(directory, { recursive: true, force: true }); }
