param([string]$Origin,[string]$RequestId,[string]$Challenge)
$ErrorActionPreference='Stop'
# Cle ephemere factice ; seule sa partie publique et une signature sortent.
$rsa=[Security.Cryptography.RSA]::Create(2048)
try {
    $options=[Text.Json.JsonSerializerOptions]::new()
    $options.Encoder=[Text.Encodings.Web.JavaScriptEncoder]::UnsafeRelaxedJsonEscaping
    $message=[Text.Json.JsonSerializer]::Serialize([object[]]@('VaultAgent.Unlock.v1',$Origin,$RequestId,$Challenge,$true,$false),$options)
    $signature=$rsa.SignData([Text.Encoding]::UTF8.GetBytes($message),[Security.Cryptography.HashAlgorithmName]::SHA256,[Security.Cryptography.RSASignaturePadding]::Pss)
    @{publicKey=[Convert]::ToBase64String($rsa.ExportSubjectPublicKeyInfo());signature=[Convert]::ToBase64String($signature)} | ConvertTo-Json -Compress
} finally { $rsa.Dispose() }
