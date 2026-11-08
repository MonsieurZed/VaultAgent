<#
But : retirer le service et ses raccourcis. Les données chiffrées restent dans ProgramData.
#>
[CmdletBinding()]
param([switch]$Elevated)
$ErrorActionPreference = 'Stop'
$pwsh = Join-Path $env:ProgramFiles 'PowerShell\7\pwsh.exe'
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
    if ($Elevated) { throw 'Élévation refusée.' }
    $process = Start-Process $pwsh -Verb RunAs -WindowStyle Hidden -ArgumentList @('-NoProfile','-File',('"{0}"' -f $PSCommandPath),'-Elevated') -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw 'Désinstallation interrompue.' }
    return
}
$root = [IO.Path]::GetFullPath((Join-Path $env:ProgramFiles 'HomelabVaultAgent'))
if ($root -ne (Join-Path $env:ProgramFiles 'HomelabVaultAgent') -or ($root -eq $env:ProgramFiles)) { throw 'Chemin de désinstallation invalide.' }
$service = Get-Service HomelabVaultAgent -ErrorAction SilentlyContinue
if ($service -and $service.Status -ne 'Stopped') { Stop-Service HomelabVaultAgent -Force; (Get-Service HomelabVaultAgent).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(15)) }
if ($service) { & (Join-Path $env:SystemRoot 'System32\sc.exe') delete HomelabVaultAgent | Out-Null; if($LASTEXITCODE -ne 0) { throw 'Suppression du service échouée.' } }
Get-CimInstance Win32_Process -Filter "Name='VaultAgent.exe'" | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith("$root\",[StringComparison]::OrdinalIgnoreCase) } | ForEach-Object {
    $process = [Diagnostics.Process]::GetProcessById($_.ProcessId); try { $process.Kill($true); $process.WaitForExit(5000) | Out-Null } finally { $process.Dispose() }
}
foreach ($directory in @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Startup'))) {
    $link = Join-Path $directory 'Coffre Bitwarden.lnk'
    if (Test-Path $link) {
        $shell = New-Object -ComObject WScript.Shell
        if ($shell.CreateShortcut($link).TargetPath.StartsWith("$root\",[StringComparison]::OrdinalIgnoreCase)) { Remove-Item -LiteralPath $link }
    }
}
if (Test-Path $root) {
    if ((Get-Item -LiteralPath $root).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Dossier lié refusé.' }
    Remove-Item -LiteralPath $root -Recurse -Force
}
Write-Host 'Service et application retirés. Données chiffrées conservées dans ProgramData\HomelabVaultAgent.'
