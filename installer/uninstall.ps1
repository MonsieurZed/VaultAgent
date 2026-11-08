<# Retirer uniquement VaultAgent. Le compte protégé et les profils restent dans ProgramData. #>
param([switch]$Elevated, [switch]$Silent, [switch]$CheckOnly)
$ErrorActionPreference = 'Stop'
if ($CheckOnly) {
    $root = [IO.Path]::GetFullPath((Join-Path $env:ProgramFiles 'HomelabVaultAgent'))
    if ((Split-Path $root -Parent) -ne $env:ProgramFiles) { throw 'Chemin de désinstallation refusé.' }
    if ((Test-Path -LiteralPath $root) -and ((Get-Item -LiteralPath $root).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Dossier lié refusé.' }
    Write-Output 'Vérification du chemin de désinstallation réussie ; aucune modification.'
    exit 0
}
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
    if ($Elevated) { throw 'Validation administrateur requise.' }
    $arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $PSCommandPath + '" -Elevated'
    if ($Silent) { $arguments += ' -Silent' }
    $child = Start-Process (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe') -Verb RunAs -WindowStyle Hidden -ArgumentList $arguments -Wait -PassThru
    exit $child.ExitCode
}
if (-not $Silent) {
    Add-Type -AssemblyName System.Windows.Forms
    if ([Windows.Forms.MessageBox]::Show('Désinstaller VaultAgent ? Les données chiffrées et le compte local seront conservés.','VaultAgent','YesNo','Question') -ne 'Yes') { exit 0 }
}
$expected = Join-Path $env:ProgramFiles 'HomelabVaultAgent'
$root = [IO.Path]::GetFullPath($expected)
if ($root -ne $expected -or (Split-Path $root -Parent) -ne $env:ProgramFiles) { throw 'Chemin de désinstallation refusé.' }
if ((Get-Item -LiteralPath $root).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Dossier lié refusé.' }
$service = Get-Service HomelabVaultAgent -ErrorAction SilentlyContinue
if ($service -and $service.Status -ne 'Stopped') { Stop-Service HomelabVaultAgent -Force; (Get-Service HomelabVaultAgent).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(20)) }
Get-CimInstance Win32_Process -Filter "Name='VaultAgent.exe'" | Where-Object { $_.ExecutablePath -eq (Join-Path $root 'VaultAgent.exe') } | ForEach-Object {
    $candidateId = $_.ProcessId
    try { $child = [Diagnostics.Process]::GetProcessById($candidateId); try { $child.Kill($true); $child.WaitForExit(5000) | Out-Null } finally { $child.Dispose() } }
    catch { if(Get-Process -Id $candidateId -ErrorAction SilentlyContinue){throw} }
}
if ($service) { & (Join-Path $env:SystemRoot 'System32\sc.exe') delete HomelabVaultAgent | Out-Null; if($LASTEXITCODE -ne 0){throw 'Suppression du service échouée.'} }
$shell = New-Object -ComObject WScript.Shell
foreach ($directory in @([Environment]::GetFolderPath('DesktopDirectory'),[Environment]::GetFolderPath('Startup'),[Environment]::GetFolderPath('Programs'))) {
    $link = Join-Path $directory 'Coffre Bitwarden.lnk'
    if ((Test-Path -LiteralPath $link) -and $shell.CreateShortcut($link).TargetPath -eq (Join-Path $root 'VaultAgent.exe')) { Remove-Item -LiteralPath $link -Force }
}
Remove-Item -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\HomelabVaultAgent' -Recurse -Force -ErrorAction SilentlyContinue
if ([IO.Path]::GetFullPath($root) -ne (Join-Path $env:ProgramFiles 'HomelabVaultAgent')) { throw 'Chemin final de désinstallation refusé.' }
Remove-Item -LiteralPath $root -Recurse -Force
