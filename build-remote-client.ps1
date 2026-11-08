<# Paquet portable : aucun service installe ou requis sur le client Windows. #>
$ErrorActionPreference = 'Stop'
$output = Join-Path $PSScriptRoot 'dist\windows-client'
if (Test-Path -LiteralPath (Join-Path $output 'client-manifest.json')) { Remove-Item -LiteralPath (Join-Path $output 'client-manifest.json') }
& dotnet publish (Join-Path $PSScriptRoot 'client\Client.csproj') -c Release -r win-x64 --self-contained true -p:RuntimeFrameworkVersion=10.0.14 -o $output --nologo
if ($LASTEXITCODE -ne 0) { throw 'Lanceur client non compile.' }
if (Test-Path (Join-Path $output 'VaultAgent.exe')) { throw 'Le client portable ne doit pas contenir le service autonome.' }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'remote-client\agent-tools.mjs') -Destination (Join-Path $output 'remote-agent-tools.mjs') -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'remote-client\README.md') -Destination $output -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'LICENSE') -Destination $output -Force
$archive = Join-Path $PSScriptRoot 'dist\VaultAgent-Windows-Client-3.0.0-preview.2-x64.zip'
if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive }
[IO.Compression.ZipFile]::CreateFromDirectory($output,$archive,[IO.Compression.CompressionLevel]::Optimal,$false)
$hash = (Get-FileHash -LiteralPath $archive).Hash.ToLowerInvariant()
"$hash  $([IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath ($archive + '.sha256') -Encoding ascii
Write-Output "Client portable pret : $archive"
