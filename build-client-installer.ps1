<# Installateur du client utilisateur, sans service SYSTEM et sans compte embarque. #>
param([switch]$SkipClientBuild)
$ErrorActionPreference='Stop'
if(-not $SkipClientBuild){& (Join-Path $PSScriptRoot 'build-remote-client.ps1'); if($LASTEXITCODE -ne 0){throw 'Client non compile.'}}
$package=Join-Path $PSScriptRoot 'dist\windows-client'
$distribution=Join-Path $PSScriptRoot 'dist'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'client-setup\uninstall.ps1') -Destination $package -Force
$files=@{}
foreach($file in [IO.Directory]::EnumerateFiles($package,'*',[IO.SearchOption]::AllDirectories)){
  if([IO.Path]::GetFileName($file) -eq 'client-manifest.json'){continue}
  $relative=[IO.Path]::GetRelativePath($package,$file).Replace('\','/')
  $files[$relative]=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
}
$files | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $package 'client-manifest.json') -Encoding utf8
$payload=Join-Path $distribution 'client-payload.zip'
if(Test-Path -LiteralPath $payload){Remove-Item -LiteralPath $payload}
[IO.Compression.ZipFile]::CreateFromDirectory($package,$payload,[IO.Compression.CompressionLevel]::Optimal,$false)
$build=Join-Path $distribution ('client-setup-build-'+[guid]::NewGuid().ToString('N'))
for($attempt=0;$attempt -lt 3;$attempt++){
  & dotnet publish (Join-Path $PSScriptRoot 'client-setup\Setup.csproj') -c Release -r win-x64 --self-contained true -p:RuntimeFrameworkVersion=10.0.14 -o $build --nologo
  if($LASTEXITCODE -eq 0){break}
  if($attempt -lt 2){Start-Sleep -Milliseconds 500}
}
if($LASTEXITCODE -ne 0){throw 'Installateur non compile.'}
& dotnet (Join-Path $PSScriptRoot 'client-setup\bin\Release\net9.0-windows\win-x64\VaultAgent-Client-Setup.dll') --verify-package
if($LASTEXITCODE -ne 0){throw 'Verification du paquet echouee.'}
$installer=Join-Path $distribution 'VaultAgent-Client-Setup-3.0.0-preview.2-x64.exe'
Copy-Item -LiteralPath (Join-Path $build 'VaultAgent-Client-Setup.exe') -Destination $installer -Force
$hash=(Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
"$hash  $([IO.Path]::GetFileName($installer))" | Set-Content -LiteralPath ($installer+'.sha256') -Encoding ascii
Write-Output "Installateur client pret et paquet verifie : $installer"
