<# Construire l'identité MSIX ; aucune confiance n'est ajoutée au système par ce script. #>
[CmdletBinding()]
param([string]$CertificateThumbprint, [switch]$DevelopmentCertificate,
    [string]$SdkBin = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64')
$ErrorActionPreference = 'Stop'
$output = Join-Path $PSScriptRoot 'dist'
$staging = Join-Path $output 'passkey-identity'
New-Item -ItemType Directory -Path $staging -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Passkeys\Package\AppxManifest.xml') -Destination (Join-Path $staging 'AppxManifest.xml') -Force
$packageVersion = ([xml](Get-Content (Join-Path $staging 'AppxManifest.xml') -Raw)).Package.Identity.Version
$package = Join-Path $output ('VaultAgent-Passkeys-' + ([version]$packageVersion).ToString(3) + '.msix')
$makeappx = Join-Path $SdkBin 'makeappx.exe'
$signtool = Join-Path $SdkBin 'signtool.exe'
if (-not (Test-Path $makeappx)) { throw 'Windows SDK et MakeAppx requis.' }
& $makeappx pack /o /d $staging /nv /p $package | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Construction de l'identité Windows échouée." }
if ($DevelopmentCertificate -and -not $CertificateThumbprint) {
    $saved = Join-Path $PSScriptRoot '.passkey-sdk\signing-certificate.json'
    if (Test-Path $saved) { $CertificateThumbprint = (Get-Content $saved -Raw | ConvertFrom-Json).thumbprint }
    if (-not $CertificateThumbprint -or -not (Test-Path "Cert:\CurrentUser\My\$CertificateThumbprint")) {
        $certificate = New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=MonsieurZed VaultAgent' -CertStoreLocation 'Cert:\CurrentUser\My' -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 -KeyExportPolicy NonExportable -NotAfter ([DateTime]::Now.AddYears(2)) -FriendlyName 'VaultAgent - signature locale de développement'
        $CertificateThumbprint = $certificate.Thumbprint
        New-Item -ItemType Directory -Path (Split-Path $saved -Parent) -Force | Out-Null
        @{thumbprint=$CertificateThumbprint} | ConvertTo-Json | Set-Content -LiteralPath $saved -Encoding utf8
    }
}
if (-not $CertificateThumbprint) { Write-Output "Identité non signée : $package"; return }
$certificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$CertificateThumbprint"
if ($certificate.Subject -ne 'CN=MonsieurZed VaultAgent' -or -not $certificate.HasPrivateKey) { throw 'Certificat de signature different du manifeste.' }
& $signtool sign /fd SHA256 /sha1 $CertificateThumbprint /s My $package | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Signature de l'identité Windows échouée." }
$publicCertificate = Join-Path $output 'VaultAgent-Passkeys.cer'
[IO.File]::WriteAllBytes($publicCertificate,$certificate.Export([Security.Cryptography.X509Certificates.X509ContentType]::Cert))
@{packageName='MonsieurZed.VaultAgent.Passkeys';packageVersion=$packageVersion;publisher=$certificate.Subject;packageFile=[IO.Path]::GetFileName($package);
  packageSha256=(Get-FileHash $package).Hash;certificateFile=[IO.Path]::GetFileName($publicCertificate);certificateSha256=(Get-FileHash $publicCertificate).Hash;
  certificateThumbprint=$certificate.Thumbprint;development=[bool]$DevelopmentCertificate} | ConvertTo-Json | Set-Content (Join-Path $output 'passkey-package.json') -Encoding utf8
Write-Output "Identité signée prête : $package ; certificat public exporté ; aucune confiance ajoutée."
