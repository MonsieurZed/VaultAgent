/* But : compiler le fork local avec les services upstream inchangés. */
const path = require('node:path');
const fs = require('node:fs');
const webpack = require('webpack');
const [source, dependencies, output] = process.argv.slice(2);
if (!source || !dependencies || !output) throw new Error('source, dependencies, output required');
process.chdir(path.join(source, 'apps/cli'));
const { buildConfig } = require(path.join(source, 'apps/cli/webpack.base.js'));
const config = buildConfig({ configName: 'Homelab Agent GPL',
  entry: path.join(source, 'apps/cli/src/homelab-agent.ts'), tsConfig: path.join(source, 'tsconfig.base.json'),
  outputPath: output, mode: 'production', env: 'production',
  modulesPath: [dependencies], externalsModulesDir: dependencies,
  localesPath: path.join(source, 'apps/cli/src/locales'),
  importAliases: { '@agent-upstream': path.join(source, 'apps/cli/src') } });
config.module.rules[0].use = { loader: path.join(dependencies, 'ts-loader'), options: {
  transpileOnly: true, configFile: path.join(source, 'tsconfig.base.json') } };
config.resolveLoader = { modules: [dependencies] };
config.devtool = false;
webpack(config, (error, stats) => {
  if (error || stats.hasErrors()) {
    process.stderr.write(error ? String(error) : stats.toString({ all: false, errors: true }));
    process.exitCode = 1;
  } else {
    fs.copyFileSync(path.join(source, 'LICENSE_GPL.txt'), path.join(output, 'LICENSE-GPL.txt'));
    fs.writeFileSync(path.join(output, 'upstream.json'), JSON.stringify({
      repository: 'https://github.com/bitwarden/clients', tag: 'cli-v2026.10.1',
      commit: '91214c3b978edb5496a32a946451960fe7d44531', adaptation: 'fork/agent.ts' }, null, 2));
    process.stdout.write(stats.toString({ all: false, timings: true, errors: true }) + '\n');
  }
});
