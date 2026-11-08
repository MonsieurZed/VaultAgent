<# Enregistrer uniquement l'identité du fournisseur ; confiance de développement sur demande explicite. #>
[CmdletBinding()]
param([switch]$TrustDevelopmentCertificate, [switch]$Uninstall)
$ErrorActionPreference = 'Stop'
$name = 'MonsieurZed.VaultAgent.Passkeys'
$registry = 'HKCU:\Software\MonsieurZed\VaultAgent\Passkeys'
if ($Uninstall) {
    Get-AppxPackage -Name $name | ForEach-Object { Remove-AppxPackage -Package $_.PackageFullName }
    if (Test-Path $registry) {
        $state = Get-ItemProperty $registry
        if ($state.CertificateAdded -eq 1 -and $state.CertificateThumbprint -match '^[A-Fa-f0-9]{40}$') {
            $path = 'Cert:\CurrentUser\TrustedPeople\' + $state.CertificateThumbprint
            if ((Test-Path $path) -and (Get-Item $path).Subject -eq 'CN=MonsieurZed VaultAgent') { Remove-Item -LiteralPath $path }
        }
        Remove-Item -LiteralPath $registry
    }
    exit 0
}
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ($root -ne (Join-Path $env:ProgramFiles 'HomelabVaultAgent')) { throw 'Installer le fournisseur dans Program Files avant son enregistrement.' }
$metadata = Get-Content (Join-Path $PSScriptRoot 'passkey-package.json') -Raw | ConvertFrom-Json
if ($metadata.packageName -ne $name -or $metadata.publisher -ne 'CN=MonsieurZed VaultAgent') { throw 'Identité du package inattendue.' }
foreach ($file in @($metadata.packageFile,$metadata.certificateFile)) {
    if ([IO.Path]::GetFileName($file) -ne $file) { throw 'Nom de fichier de package invalide.' }
}
$package = Join-Path $PSScriptRoot $metadata.packageFile
$certificate = Join-Path $PSScriptRoot $metadata.certificateFile
if ((Get-FileHash $package).Hash -ne $metadata.packageSha256 -or (Get-FileHash $certificate).Hash -ne $metadata.certificateSha256) { throw 'Empreinte du package ou du certificat différente.' }
$public = New-Object Security.Cryptography.X509Certificates.X509Certificate2($certificate)
if ($public.Thumbprint -ne $metadata.certificateThumbprint -or $public.Subject -ne $metadata.publisher) { throw 'Certificat de package différent.' }
$added = $false
if ($TrustDevelopmentCertificate) {
    if (-not $metadata.development) { throw 'Cette option est réservée au package de développement.' }
    $path = 'Cert:\CurrentUser\TrustedPeople\' + $public.Thumbprint
    if (-not (Test-Path $path)) { Import-Certificate -FilePath $certificate -CertStoreLocation 'Cert:\CurrentUser\TrustedPeople' | Out-Null; $added = $true }
}
try {
    $existing = Get-AppxPackage -Name $name
    if (-not $existing -or $existing.Version -ne $metadata.packageVersion) { Add-AppxPackage -Path $package -ExternalLocation $root }
    $current = Get-AppxPackage -Name $name
    if (-not $current) { throw 'Identité Windows non enregistrée.' }
    $previousAdded = 0
    if (Test-Path $registry) {
        $previous = Get-ItemProperty $registry
        if ($previous.CertificateThumbprint -eq $public.Thumbprint) { $previousAdded = $previous.CertificateAdded }
    }
    New-Item -Path $registry -Force | Out-Null
    Set-ItemProperty $registry PackageFamilyName $current.PackageFamilyName
    Set-ItemProperty $registry CertificateThumbprint $public.Thumbprint
    Set-ItemProperty $registry CertificateAdded ([int]($added -or $previousAdded -eq 1))
    Write-Output 'Identité du fournisseur enregistrée.'
} catch {
    if ($added) { Remove-Item -LiteralPath ('Cert:\CurrentUser\TrustedPeople\' + $public.Thumbprint) }
    throw
}
