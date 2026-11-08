# Certificat éphémère du serveur de test ; jamais installé dans un magasin Windows.
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
$key = [Security.Cryptography.RSA]::Create(2048)
try {
    $request = [Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=127.0.0.1',$key,[Security.Cryptography.HashAlgorithmName]::SHA256,[Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $san = [Security.Cryptography.X509Certificates.SubjectAlternativeNameBuilder]::new()
    $san.AddIpAddress([Net.IPAddress]::Loopback)
    $request.CertificateExtensions.Add($san.Build())
    $request.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($true,$false,0,$true))
    $certificate = $request.CreateSelfSigned([DateTimeOffset]::UtcNow.AddMinutes(-5),[DateTimeOffset]::UtcNow.AddHours(1))
    try {
        [IO.File]::WriteAllText((Join-Path $Directory 'test.crt'),$certificate.ExportCertificatePem())
        [IO.File]::WriteAllText((Join-Path $Directory 'test.key'),$key.ExportPkcs8PrivateKeyPem())
    } finally { $certificate.Dispose() }
} finally { $key.Dispose() }
