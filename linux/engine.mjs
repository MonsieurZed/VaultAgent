/* Adaptateur du moteur persistant existant ; les sorties d'erreur sont drainees sans journalisation. */
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { join } from 'node:path';

export class Engine {
  constructor(directory, entry) { this.directory = directory; this.entry = entry; this.sequence = 0; this.pending = new Map(); }
  get alive() { return !!this.child && this.child.exitCode === null && !this.child.killed; }
  async start(profile) {
    const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => !name.startsWith('NODE_') && !name.startsWith('BW_') && name !== 'BITWARDENCLI_APPDATA_DIR'));
    env.BITWARDENCLI_APPDATA_DIR = join(this.directory, 'profiles', profile); env.BW_NOINTERACTION = 'true';
    this.child = spawn(process.execPath, [this.entry], { env, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
    this.child.stderr.resume();
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.kill(); reject(new Error('engine-timeout')); }, 60000);
      const fail = () => { clearTimeout(timer); reject(new Error('engine-stopped')); for (const pending of this.pending.values()) pending.reject(new Error('engine-stopped')); this.pending.clear(); };
      this.child.once('error', fail); this.child.once('exit', fail);
      createInterface({ input: this.child.stdout }).on('line', line => {
        let result; try { if (line.length > 32 * 1024 * 1024) throw new Error(); result = JSON.parse(line); } catch { this.kill(); fail(); return; }
        if ('ready' in result) { clearTimeout(timer); result.ready ? resolve() : reject(new Error('engine-init')); return; }
        const pending = this.pending.get(result.id); if (!pending) return; this.pending.delete(result.id);
        result.success ? pending.resolve(result.data) : pending.reject(new Error(/^[a-z-]+:[a-z-]+$/.test(result.code || '') ? result.code : 'engine-refused'));
      });
    }); return this;
  }
  async request(request) {
    if (!this.child || this.child.exitCode !== null || this.child.killed) throw new Error('engine-locked');
    return new Promise((resolve, reject) => {
      const id = ++this.sequence;
      const timer = setTimeout(() => { this.pending.delete(id); this.kill(); reject(new Error('engine-timeout')); }, 90000);
      this.pending.set(id, { resolve: value => { clearTimeout(timer); resolve(value); }, reject: error => { clearTimeout(timer); reject(error); } });
      this.child.stdin.write(JSON.stringify({ ...request, id }) + '\n', error => { if (error) { this.pending.delete(id); clearTimeout(timer); reject(new Error('engine-write')); } });
    });
  }
  kill() { const child = this.child; this.child = null; if (child && !child.killed) child.kill(); }
}
