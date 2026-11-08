<#
But : produire un paquet Windows complet et vérifié avant de demander l'élévation d'installation.
#>
param([switch]$SelfContained)
$ErrorActionPreference = 'Stop'
$project = Join-Path $PSScriptRoot 'VaultAgent.csproj'
$package = Join-Path $PSScriptRoot 'publish'
$publishOptions = @()
if ($SelfContained) { $publishOptions += '-p:RuntimeFrameworkVersion=10.0.14' }
& dotnet publish $project -c Release -r win-x64 --self-contained $SelfContained.IsPresent.ToString().ToLowerInvariant() @publishOptions -o $package --nologo
if ($LASTEXITCODE -ne 0) { throw 'Publication .NET échouée.' }
if (-not (Test-Path (Join-Path $PSScriptRoot 'runtime\engine\bw.js'))) { throw 'Construire le moteur avec build-engine.ps1.' }
& (Join-Path $env:SystemRoot 'System32\robocopy.exe') (Join-Path $PSScriptRoot 'runtime') (Join-Path $package 'runtime') /E /COPY:DAT /DCOPY:DAT /R:1 /W:1 /XD tests /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw 'Copie du moteur incomplète.' }
New-Item -ItemType Directory -Path (Join-Path $package 'sources') -Force | Out-Null
foreach ($name in @('agent.ts','build.cjs')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot "fork\$name") -Destination (Join-Path $package 'sources') -Force }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'build-engine.ps1') -Destination (Join-Path $package 'sources') -Force
& dotnet (Join-Path $package 'VaultAgent.dll') --self-test
if ($LASTEXITCODE -ne 0) { throw "Vérifications de l'application échouées." }
$manifest = @{}
foreach ($name in @('VaultAgent.exe','VaultAgent.dll','agent-tools.mjs','runtime\node.exe','runtime\engine\bw.js')) {
    $manifest[$name] = (Get-FileHash -LiteralPath (Join-Path $package $name) -Algorithm SHA256).Hash
}
@{ version = '2.0.0'; createdUtc = [DateTime]::UtcNow.ToString('o'); files = $manifest } |
    ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $package 'package-manifest.json') -Encoding utf8
Write-Host "Paquet prêt : $package"
