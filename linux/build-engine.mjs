/* Construction Linux du meme fork et des memes dependances epinglees que Windows. */
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync, copyFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
const root = resolve('.'); const source = join(root, '.upstream'); const build = join(root, '.build-deps'); const runtime = join(root, 'runtime');
function run(file, args, cwd = root, env = process.env) { const result = spawnSync(file, args, { cwd, env, stdio: 'inherit' }); if (result.status !== 0) throw new Error('Build command failed'); }
run('git', ['clone', '--filter=blob:none', '--no-checkout', '--branch', 'cli-v2026.10.1', 'https://github.com/bitwarden/clients.git', source]);
run('git', ['sparse-checkout', 'set', 'apps/cli', 'libs', 'scripts'], source);
run('git', ['checkout', '--detach', '91214c3b978edb5496a32a946451960fe7d44531'], source);
const revision = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: source, encoding: 'utf8' });
if (revision.stdout.trim() !== '91214c3b978edb5496a32a946451960fe7d44531') throw new Error('Unexpected upstream revision');
copyFileSync('fork/agent.ts', join(source, 'apps/cli/src/homelab-agent.ts'));
copyFileSync('fork/write-patch.mjs', join(source, 'apps/cli/src/homelab-write-patch.mjs'));
const upstream = JSON.parse(readFileSync(join(source, 'package.json'))); const cli = JSON.parse(readFileSync(join(source, 'apps/cli/package.json')));
mkdirSync(build, { recursive: true }); mkdirSync(runtime, { recursive: true });
const dependencies = { ...upstream.dependencies, ...cli.dependencies }; delete dependencies['@bitwarden/desktop-napi'];
const names = ['webpack', 'webpack-cli', 'typescript', 'ts-loader', 'copy-webpack-plugin', 'webpack-node-externals', 'tsconfig-paths-webpack-plugin'];
writeFileSync(join(build, 'package.json'), JSON.stringify({ name: 'homelab-vault-engine-build', version: '1.0.0', private: true, dependencies, devDependencies: Object.fromEntries(names.map(name => [name, upstream.devDependencies[name]])) }));
copyFileSync('fork/build-deps.lock.json', join(build, 'package-lock.json')); run('npm', ['ci', '--ignore-scripts', '--no-audit', '--no-fund'], build);
run(process.execPath, [join(root, 'fork/build.cjs'), source, join(build, 'node_modules'), join(runtime, 'engine')], root, { ...process.env, NODE_PATH: join(build, 'node_modules') });
writeFileSync(join(runtime, 'package.json'), JSON.stringify({ name: 'homelab-vault-engine-runtime', version: '2.0.0', private: true, dependencies: cli.dependencies }));
copyFileSync('fork/runtime.lock.json', join(runtime, 'package-lock.json')); run('npm', ['ci', '--ignore-scripts', '--omit=dev', '--no-audit', '--no-fund'], runtime);
