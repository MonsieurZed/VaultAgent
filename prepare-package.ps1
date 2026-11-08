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
$providerOutput = Join-Path $PSScriptRoot 'dist\provider-build'
& dotnet publish (Join-Path $PSScriptRoot 'provider\Provider.csproj') -c Release -r win-x64 --self-contained $SelfContained.IsPresent.ToString().ToLowerInvariant() @publishOptions -o $providerOutput --nologo
if ($LASTEXITCODE -ne 0) { throw 'Compilation du lanceur de passkeys echouee.' }
foreach ($suffix in @('exe','dll','deps.json','runtimeconfig.json')) {
    Copy-Item -LiteralPath (Join-Path $providerOutput "VaultAgent.Provider.$suffix") -Destination $package -Force
}
if (-not (Test-Path (Join-Path $PSScriptRoot 'runtime\engine\bw.js'))) { throw 'Construire le moteur avec build-engine.ps1.' }
& (Join-Path $env:SystemRoot 'System32\robocopy.exe') (Join-Path $PSScriptRoot 'runtime') (Join-Path $package 'runtime') /E /COPY:DAT /DCOPY:DAT /R:1 /W:1 /XD tests /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw 'Copie du moteur incomplète.' }
New-Item -ItemType Directory -Path (Join-Path $package 'sources') -Force | Out-Null
foreach ($name in @('agent.ts','build.cjs')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot "fork\$name") -Destination (Join-Path $package 'sources') -Force }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'build-engine.ps1') -Destination (Join-Path $package 'sources') -Force
& dotnet (Join-Path $package 'VaultAgent.dll') --self-test
if ($LASTEXITCODE -ne 0) { throw "Vérifications de l'application échouées." }
& dotnet (Join-Path $package 'VaultAgent.dll') --write-passkey-assets (Join-Path $package 'assets')
if ($LASTEXITCODE -ne 0) { throw 'Generation des icones passkeys echouee.' }
$identityMetadata = Join-Path $PSScriptRoot 'dist\passkey-package.json'
if (Test-Path -LiteralPath $identityMetadata) {
    $identity = Get-Content -LiteralPath $identityMetadata -Raw | ConvertFrom-Json
    $passkeyDirectory = [IO.Path]::GetFullPath((Join-Path $package 'passkeys'))
    foreach ($old in Get-ChildItem -LiteralPath $passkeyDirectory -Filter 'VaultAgent-Passkeys-*.msix' -File) {
        if ([IO.Path]::GetDirectoryName($old.FullName) -ne $passkeyDirectory) { throw 'Chemin du package inattendu.' }
        Remove-Item -LiteralPath $old.FullName
    }
    foreach ($entry in @(@($identity.packageFile,$identity.packageSha256),@($identity.certificateFile,$identity.certificateSha256))) {
        if ([IO.Path]::GetFileName($entry[0]) -ne $entry[0]) { throw 'Nom de fichier du fournisseur invalide.' }
        $source = Join-Path $PSScriptRoot ('dist\' + $entry[0])
        if ((Get-FileHash -LiteralPath $source).Hash -ne $entry[1]) { throw 'Package passkeys different de son manifeste.' }
        Copy-Item -LiteralPath $source -Destination (Join-Path $package 'passkeys') -Force
    }
    Copy-Item -LiteralPath $identityMetadata -Destination (Join-Path $package 'passkeys') -Force
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Passkeys\MICROSOFT-LICENSE.txt') -Destination (Join-Path $package 'passkeys') -Force
$manifest = @{}
foreach ($name in @('VaultAgent.exe','VaultAgent.dll','agent-tools.mjs','runtime\node.exe','runtime\engine\bw.js')) {
    $manifest[$name] = (Get-FileHash -LiteralPath (Join-Path $package $name) -Algorithm SHA256).Hash
}
@{ version = ([xml](Get-Content $project -Raw)).Project.PropertyGroup.Version; createdUtc = [DateTime]::UtcNow.ToString('o'); files = $manifest } |
    ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $package 'package-manifest.json') -Encoding utf8
Write-Host "Paquet prêt : $package"
