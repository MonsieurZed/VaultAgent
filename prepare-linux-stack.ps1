<# Copier uniquement les sources publiques necessaires, jamais les profils ou fichiers generes. #>
param([string]$Stack = (Join-Path (Split-Path $PSScriptRoot -Parent) 'Reseau\stacks\maison\vaultagent'))
$ErrorActionPreference = 'Stop'
$context = Join-Path $Stack 'source'
New-Item -ItemType Directory -Path $context -Force | Out-Null
foreach ($directory in @('linux','fork','remote-client')) {
    New-Item -ItemType Directory -Path (Join-Path $context $directory) -Force | Out-Null
    Copy-Item -Path (Join-Path $PSScriptRoot "$directory\*") -Destination (Join-Path $context $directory) -Recurse -Force
}
foreach ($name in @('LICENSE','LICENSE-GPL.txt','THIRD-PARTY.md')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $context -Force }
@'
*
!fork/
!fork/**
!linux/
!linux/**
!remote-client/
!remote-client/**
!LICENSE
!LICENSE-GPL.txt
!THIRD-PARTY.md
'@ | Set-Content -LiteralPath (Join-Path $context '.dockerignore') -Encoding utf8
Write-Output "Contexte Linux pret : $context"
