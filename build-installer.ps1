<# Construire un EXE autonome avec le SDK Microsoft existant, sans compilateur tiers. #>
[CmdletBinding()]
param([switch]$SkipPublish)
$ErrorActionPreference = 'Stop'
$distribution = Join-Path $PSScriptRoot 'dist'
$package = Join-Path $PSScriptRoot 'publish'
New-Item -ItemType Directory -Path $distribution -Force | Out-Null
if (-not $SkipPublish) {
    & (Join-Path $PSScriptRoot 'prepare-package.ps1') -SelfContained
    if ($LASTEXITCODE -ne 0) { throw 'Publication de VaultAgent échouée.' }
}
if (-not (Test-Path (Join-Path $package 'coreclr.dll'))) { throw 'Le paquet doit inclure le runtime .NET.' }
New-Item -ItemType Directory -Path (Join-Path $package 'setup') -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'installer\uninstall.ps1') -Destination (Join-Path $package 'setup\uninstall.ps1') -Force
foreach ($name in @('agent.ts','build.cjs','build-deps.lock.json','runtime.lock.json')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "fork\$name") -Destination (Join-Path $package 'sources') -Force
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'build-engine.ps1') -Destination (Join-Path $package 'sources') -Force
if (Test-Path (Join-Path $PSScriptRoot 'runtime\LICENSE-NODE.txt')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'runtime\LICENSE-NODE.txt') -Destination (Join-Path $package 'runtime\LICENSE-NODE.txt') -Force }
foreach ($name in @('LICENSE','LICENSE-GPL.txt','THIRD-PARTY.md','README.md')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $package $name) -Force }
$hashes = @{}
foreach ($file in [IO.Directory]::EnumerateFiles($package,'*',[IO.SearchOption]::AllDirectories)) {
    if ([IO.Path]::GetFileName($file) -eq 'package-manifest.json') { continue }
    $relative = [IO.Path]::GetRelativePath($package,$file).Replace('\','/')
    $inputStream = [IO.File]::OpenRead($file)
    try { $hashes[$relative] = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($inputStream)) } finally { $inputStream.Dispose() }
}
@{ version='2.0.1'; files=$hashes } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $package 'package-manifest.json') -Encoding utf8
$archive = Join-Path $distribution 'payload.zip'
if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive }
[IO.Compression.ZipFile]::CreateFromDirectory($package,$archive,[IO.Compression.CompressionLevel]::Optimal,$false)
& dotnet publish (Join-Path $PSScriptRoot 'installer\Setup.csproj') -c Release -r win-x64 --self-contained true -p:RuntimeFrameworkVersion=10.0.14 -o (Join-Path $distribution 'setup-build') --nologo
if ($LASTEXITCODE -ne 0) { throw "Compilation de l'installateur échouée." }
$installer = Join-Path $distribution 'VaultAgent-Setup-2.0.1-x64.exe'
Copy-Item -LiteralPath (Join-Path $distribution 'setup-build\VaultAgent-Setup.exe') -Destination $installer -Force
$check = Start-Process -FilePath $installer -ArgumentList '--verify-package' -WindowStyle Hidden -Wait -PassThru
if ($check.ExitCode -ne 0) { throw "Vérification du paquet embarqué échouée." }
$hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
"$hash  $([IO.Path]::GetFileName($installer))" | Set-Content -LiteralPath ($installer + '.sha256') -Encoding ascii
Write-Host "Installateur prêt et paquet vérifié : $installer"
