/* Compiler le test contre la revision upstream deja verifiee, sans acces au coffre. */
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const root = path.resolve(__dirname, '..');
const source = process.argv[2];
if (!source) throw new Error('Chemin des sources upstream requis');
const revision = spawnSync('git', ['-C', source, 'rev-parse', 'HEAD'], { encoding: 'utf8', windowsHide: true });
if (revision.status !== 0 || revision.stdout.trim() !== '91214c3b978edb5496a32a946451960fe7d44531') throw new Error('Revision upstream differente');
const dependencies = path.join(root, '.build-deps/node_modules');
if (process.env.VAULTAGENT_MODEL_TEST !== '1') {
  const result = spawnSync(process.execPath, [__filename, source], { stdio: 'inherit', windowsHide: true,
    env: { ...process.env, NODE_PATH: dependencies, VAULTAGENT_MODEL_TEST: '1' } });
  process.exit(result.status ?? 1);
}
const webpack = require(path.join(dependencies, 'webpack'));
const output = path.join(root, '.passkey-sdk/model-test');
const { buildConfig } = require(path.join(source, 'apps/cli/webpack.base.js'));
process.chdir(path.join(source, 'apps/cli'));
const config = buildConfig({ configName: 'Test passkey factice', entry: path.join(__dirname, 'passkey-models.ts'),
  tsConfig: path.join(source, 'tsconfig.base.json'), outputPath: output, mode: 'production', env: 'production',
  modulesPath: [dependencies], externalsModulesDir: dependencies,
  localesPath: path.join(source, 'apps/cli/src/locales'),
  importAliases: { '@agent-upstream': path.join(source, 'apps/cli/src') } });
config.module.rules[0].use = { loader: path.join(dependencies, 'ts-loader'), options: { transpileOnly: true, configFile: path.join(source, 'tsconfig.base.json') } };
config.resolveLoader = { modules: [dependencies] }; config.devtool = false;
webpack(config, (error, stats) => {
  if (error || stats.hasErrors()) {
    console.error(error ? String(error) : stats.toString({ all: false, errors: true })); process.exitCode = 1; return;
  }
  const result = spawnSync(process.execPath, [path.join(output, 'bw.js')], { encoding: 'utf8', windowsHide: true,
    env: { ...process.env, NODE_PATH: dependencies } });
  if (result.status !== 0) { console.error(result.stderr); process.exitCode = 1; }
  else process.stdout.write(result.stdout);
});
