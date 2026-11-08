param([string]$Message)
$ErrorActionPreference='Stop'
# Authentificateur factice ephemere : aucune API Windows Hello et aucune cle privee exportee.
$key=[Security.Cryptography.ECDsa]::Create([Security.Cryptography.ECCurve]::CreateFromFriendlyName('nistP256'))
try {
    $digest=[Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Message))
    $challenge=[Convert]::ToBase64String($digest).TrimEnd('=').Replace('+','-').Replace('/','_')
    $client=[Text.Encoding]::UTF8.GetBytes((@{type='webauthn.get';origin='https://vaultagent-client.local';challenge=$challenge;crossOrigin=$false} | ConvertTo-Json -Compress))
    $auth=[byte[]]::new(37)
    [Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes('vaultagent-client.local')).CopyTo($auth,0)
    $auth[32]=5
    $signed=[byte[]]::new(69);$auth.CopyTo($signed,0);[Security.Cryptography.SHA256]::HashData($client).CopyTo($signed,37)
    $signature=$key.SignData($signed,[Security.Cryptography.HashAlgorithmName]::SHA256,[Security.Cryptography.DSASignatureFormat]::Rfc3279DerSequence)
    $parameters=$key.ExportParameters($false);$id=[Convert]::ToBase64String([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
    @{credential=@{id=$id;keyType='EC2';first=[Convert]::ToBase64String($parameters.Q.X);second=[Convert]::ToBase64String($parameters.Q.Y)};proof=@{credentialId=$id;clientData=[Convert]::ToBase64String($client);authenticatorData=[Convert]::ToBase64String($auth);signature=[Convert]::ToBase64String($signature)}} | ConvertTo-Json -Compress -Depth 5
} finally { $key.Dispose() }
