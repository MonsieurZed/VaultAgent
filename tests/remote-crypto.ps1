<# Verifier le format Node -> .NET avec une cle et une valeur exclusivement factices. #>
param([Parameter(Mandatory)][string]$Fixture)
$ErrorActionPreference='Stop'
$test = Get-Content -LiteralPath $Fixture -Raw | ConvertFrom-Json
$rsa=[Security.Cryptography.RSA]::Create()
try {
    $privateBytes=[Convert]::FromBase64String($test.privateKey)
    $consumed=0; $rsa.ImportPkcs8PrivateKey($privateBytes,[ref]$consumed)
    $envelope=$test.envelope
    $key=$rsa.Decrypt([Convert]::FromBase64String($envelope.encryptedKey),[Security.Cryptography.RSAEncryptionPadding]::OaepSHA256)
    $clear=[byte[]]::new([Convert]::FromBase64String($envelope.data).Length)
    $aes=[Security.Cryptography.AesGcm]::new($key,16)
    try {
        $aes.Decrypt([Convert]::FromBase64String($envelope.nonce),[Convert]::FromBase64String($envelope.data),[Convert]::FromBase64String($envelope.tag),$clear)
        if([Text.Encoding]::UTF8.GetString($clear) -ne 'VaultAgent-Remote-Fictif-42!'){throw 'Valeur de test incorrecte'}
        Write-Output 'NODE_DOTNET_ENVELOPE_OK; fictitious-only'
    } finally { $aes.Dispose(); [Security.Cryptography.CryptographicOperations]::ZeroMemory($key); [Security.Cryptography.CryptographicOperations]::ZeroMemory($clear) }
} finally { $rsa.Dispose(); if($privateBytes){[Security.Cryptography.CryptographicOperations]::ZeroMemory($privateBytes)} }
