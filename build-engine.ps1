<#
But : construire le fork persistant à partir du tag vérifié, avec dépendances de compilation isolées.
#>
[CmdletBinding()]
param([string]$Source = (Join-Path $PSScriptRoot '.upstream'))
$ErrorActionPreference = 'Stop'
$expected = '91214c3b978edb5496a32a946451960fe7d44531'
if (-not (Test-Path -LiteralPath $Source)) {
    & git clone --filter=blob:none --no-checkout --branch cli-v2026.10.1 https://github.com/bitwarden/clients.git $Source
    if ($LASTEXITCODE -ne 0) { throw 'Téléchargement des sources upstream échoué.' }
    & git -C $Source sparse-checkout set apps/cli libs scripts
    if ($LASTEXITCODE -ne 0) { throw 'Sélection des sources échouée.' }
    & git -C $Source checkout --detach $expected
    if ($LASTEXITCODE -ne 0) { throw 'Extraction de la révision upstream échouée.' }
}
$actual = (& git -C $Source rev-parse HEAD).Trim()
if ($actual -ne $expected) { throw 'Révision upstream inattendue.' }
$Source = (& git -C $Source rev-parse --show-toplevel).Trim()
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fork\agent.ts') -Destination (Join-Path $Source 'apps\cli\src\homelab-agent.ts') -Force
$dependencies = Join-Path $PSScriptRoot '.build-deps'
New-Item -ItemType Directory -Path $dependencies -Force | Out-Null
$upstream = Get-Content (Join-Path $Source 'package.json') -Raw | ConvertFrom-Json -AsHashtable
$cli = Get-Content (Join-Path $Source 'apps\cli\package.json') -Raw | ConvertFrom-Json -AsHashtable
$runtime = @{} + $upstream.dependencies + @{}
$runtime.Remove('@bitwarden/desktop-napi')
foreach ($entry in $cli.dependencies.GetEnumerator()) { $runtime[$entry.Key] = $entry.Value }
$dev = @{}
foreach ($name in @('webpack', 'webpack-cli', 'typescript', 'ts-loader', 'copy-webpack-plugin', 'webpack-node-externals', 'tsconfig-paths-webpack-plugin')) {
    $dev[$name] = $upstream.devDependencies[$name]
}
@{ name = 'homelab-vault-engine-build'; version = '1.0.0'; private = $true; dependencies = $runtime; devDependencies = $dev } |
    ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $dependencies 'package.json') -Encoding utf8
Push-Location $dependencies
try {
    $lock = Join-Path $PSScriptRoot 'fork\build-deps.lock.json'
    if (Test-Path $lock) { Copy-Item -LiteralPath $lock -Destination 'package-lock.json' -Force }
    if (Test-Path 'package-lock.json') { & npm.cmd ci --ignore-scripts --no-audit --no-fund }
    else { & npm.cmd install --ignore-scripts --no-audit --no-fund }
    if ($LASTEXITCODE -ne 0) { throw 'Installation des dépendances échouée.' }
}
finally { Pop-Location }
$previous = $env:NODE_PATH
try {
    $env:NODE_PATH = Join-Path $dependencies 'node_modules'
    & node (Join-Path $PSScriptRoot 'fork\build.cjs') $Source $env:NODE_PATH (Join-Path $PSScriptRoot 'runtime\engine')
    if ($LASTEXITCODE -ne 0) { throw 'Compilation du moteur échouée.' }
}
finally { $env:NODE_PATH = $previous }
$runtimeDirectory = Join-Path $PSScriptRoot 'runtime'
@{ name = 'homelab-vault-engine-runtime'; version = '2.0.0'; private = $true; dependencies = $cli.dependencies } |
    ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $runtimeDirectory 'package.json') -Encoding utf8
Push-Location $runtimeDirectory
try {
    $lock = Join-Path $PSScriptRoot 'fork\runtime.lock.json'
    if (Test-Path $lock) { Copy-Item -LiteralPath $lock -Destination 'package-lock.json' -Force }
    if (Test-Path 'package-lock.json') { & npm.cmd ci --ignore-scripts --omit=dev --no-audit --no-fund }
    else { & npm.cmd install --ignore-scripts --omit=dev --no-audit --no-fund }
    if ($LASTEXITCODE -ne 0) { throw 'Installation du runtime échouée.' }
}
finally { Pop-Location }
Copy-Item -LiteralPath (Get-Command node.exe -ErrorAction Stop).Source -Destination (Join-Path $runtimeDirectory 'node.exe') -Force
$nodeLicense = Join-Path (Split-Path (Get-Command node.exe).Source -Parent) 'LICENSE'
if (Test-Path $nodeLicense) { Copy-Item -LiteralPath $nodeLicense -Destination (Join-Path $runtimeDirectory 'LICENSE-NODE.txt') -Force }
