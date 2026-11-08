<#
But : installer le service système et l'icône utilisateur après validation UAC.
Les données des anciens profils et les terminaux d'import restent conservés.
#>
[CmdletBinding()]
param([switch]$Elevated, [string]$OwnerSid = ([Security.Principal.WindowsIdentity]::GetCurrent().User.Value))
$ErrorActionPreference = 'Stop'
$package = Join-Path $PSScriptRoot 'publish'
$root = Join-Path $env:ProgramFiles 'HomelabVaultAgent'
$dataRoot = Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'HomelabVaultAgent'
$pwsh = Join-Path $env:ProgramFiles 'PowerShell\7\pwsh.exe'
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
function Write-InstallStage([string]$Stage, [string]$Message = '') {
    @{ stage=$Stage; message=$Message; elevated=$isAdmin; date=[DateTime]::UtcNow.ToString('o') } | ConvertTo-Json |
        Set-Content -LiteralPath (Join-Path $PSScriptRoot 'install-diagnostic.json') -Encoding utf8
}
Write-InstallStage 'started'
if (-not $isAdmin) {
    if ($Elevated) { throw 'Élévation refusée.' }
    try {
        $process = Start-Process $pwsh -Verb RunAs -WindowStyle Hidden -ArgumentList @('-NoProfile','-STA','-File',('"{0}"' -f $PSCommandPath),'-Elevated','-OwnerSid',$OwnerSid) -Wait -PassThru
    }
    catch { Write-InstallStage 'elevation-failed' $_.Exception.Message; throw }
    if ($process.ExitCode -ne 0) { throw 'Installation interrompue ; consulter la fenêtre de résultat.' }
    Start-Process -FilePath (Join-Path $root 'VaultAgent.exe') -ArgumentList '--tray' | Out-Null
    return
}
try {
    Write-InstallStage 'elevated-start'
    if ([Security.Principal.WindowsIdentity]::GetCurrent().User.Value -ne $OwnerSid) { throw "Utiliser le même compte Windows pour l'élévation." }
    $manifest = Get-Content -LiteralPath (Join-Path $package 'package-manifest.json') -Raw | ConvertFrom-Json -AsHashtable
    foreach ($entry in $manifest.files.GetEnumerator()) {
        if ((Get-FileHash -LiteralPath (Join-Path $package $entry.Key)).Hash -ne $entry.Value) { throw 'Le paquet a changé depuis sa vérification.' }
    }
    $service = Get-Service -Name HomelabVaultAgent -ErrorAction SilentlyContinue
    Write-InstallStage 'verified-package'
    if ($service -and $service.Status -ne 'Stopped') {
        $serviceId = (Get-CimInstance Win32_Service -Filter "Name='HomelabVaultAgent'").ProcessId
        $serviceProcess = if($serviceId) { [Diagnostics.Process]::GetProcessById($serviceId) } else { $null }
        try {
            Stop-Service -Name HomelabVaultAgent -Force; (Get-Service HomelabVaultAgent).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(15))
            if ($serviceProcess -and -not $serviceProcess.WaitForExit(10000)) { $serviceProcess.Kill($true); $serviceProcess.WaitForExit(5000) | Out-Null }
        }
        finally { if($serviceProcess){$serviceProcess.Dispose()} }
    }
    $oldRoot = Join-Path $env:ProgramFiles 'HomelabVault'
    Get-CimInstance Win32_Process -Filter "Name='pwsh.exe' OR Name='VaultAgent.exe'" | Where-Object {
        $_.ProcessId -ne $PID -and $_.SessionId -eq (Get-Process -Id $PID).SessionId -and
        (($_.Name -eq 'VaultAgent.exe') -or
         ($_.CommandLine -and $_.CommandLine.Contains($oldRoot) -and $_.CommandLine -match '\s-Action\s+(Setup|Agent|Tray)(\s|$)'))
    } | ForEach-Object {
        $candidateId = $_.ProcessId
        try {
            $owner = Invoke-CimMethod -InputObject $_ -MethodName GetOwnerSid
            if ($owner.Sid -eq $OwnerSid) { $child = [Diagnostics.Process]::GetProcessById($candidateId); try { $child.Kill($true); $child.WaitForExit(5000) | Out-Null } finally { $child.Dispose() } }
        }
        catch { if(Get-Process -Id $candidateId -ErrorAction SilentlyContinue){throw} }
    }
    foreach ($directory in @($root,$dataRoot)) {
        if ((Test-Path $directory) -and ((Get-Item -LiteralPath $directory).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Dossier lié refusé.' }
        if (Test-Path $directory) {
            $owner = (Get-Acl -LiteralPath $directory).GetOwner([Security.Principal.SecurityIdentifier]).Value
            if ($owner -notin @('S-1-5-18','S-1-5-32-544')) { throw 'Propriétaire du dossier existant inattendu.' }
        }
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
        $acl = [Security.AccessControl.DirectorySecurity]::new(); $acl.SetAccessRuleProtection($true,$false)
        $acl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
        foreach ($sid in @('S-1-5-18','S-1-5-32-544')) { $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($sid),'FullControl','ContainerInherit,ObjectInherit','None','Allow')) }
        if ($directory -eq $root) { $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new('S-1-5-32-545'),'ReadAndExecute','ContainerInherit,ObjectInherit','None','Allow')) }
        Set-Acl -LiteralPath $directory -AclObject $acl
    }
    Write-InstallStage 'copying-files'
    for ($attempt = 0; $attempt -lt 3; $attempt++) {
        Get-CimInstance Win32_Process -Filter "Name='VaultAgent.exe'" | Where-Object {
            $_.ExecutablePath -eq (Join-Path $root 'VaultAgent.exe')
        } | ForEach-Object {
            $candidateId = $_.ProcessId
            try {
                $running = [Diagnostics.Process]::GetProcessById($candidateId)
                try { $running.Kill($true); $running.WaitForExit(5000) | Out-Null } finally { $running.Dispose() }
            } catch { if(Get-Process -Id $candidateId -ErrorAction SilentlyContinue){throw} }
        }
        & (Join-Path $env:SystemRoot 'System32\robocopy.exe') $package $root /E /COPY:DAT /DCOPY:DAT /R:0 /W:0 /NFL /NDL /NJH /NJS /NP ("/UNILOG:{0}" -f (Join-Path $PSScriptRoot 'install-copy.log')) | Out-Null
        if ($LASTEXITCODE -lt 8) { break }
        if ($attempt -eq 2) { throw 'Installation des fichiers incomplète.' }
    }
    & (Join-Path $env:SystemRoot 'System32\icacls.exe') $root /setowner '*S-1-5-32-544' /T /Q | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Protection des fichiers incomplète.' }
    $oldCertificate = Join-Path $oldRoot 'localhost.crt'
    if (Test-Path $oldCertificate) { Copy-Item -LiteralPath $oldCertificate -Destination (Join-Path $root 'localhost.crt') -Force }
    foreach ($entry in $manifest.files.GetEnumerator()) { if ((Get-FileHash -LiteralPath (Join-Path $root $entry.Key)).Hash -ne $entry.Value) { throw 'Vérification des fichiers installés échouée.' } }
    $binary = '"' + (Join-Path $root 'VaultAgent.exe') + '" --service'
    Write-InstallStage 'registering-service'
    if (-not $service) { New-Service -Name HomelabVaultAgent -DisplayName 'Coffre Bitwarden · Agent local' -BinaryPathName $binary -StartupType Automatic | Out-Null }
    else { & (Join-Path $env:SystemRoot 'System32\sc.exe') config HomelabVaultAgent binPath= $binary start= auto | Out-Null; if ($LASTEXITCODE -ne 0) { throw 'Configuration du service échouée.' } }
    Start-Service -Name HomelabVaultAgent
    (Get-Service HomelabVaultAgent).WaitForStatus('Running',[TimeSpan]::FromSeconds(15))
    Write-InstallStage 'service-running'
    $shell = New-Object -ComObject WScript.Shell
    foreach ($directory in @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Startup'))) {
        $shortcut = $shell.CreateShortcut((Join-Path $directory 'Coffre Bitwarden.lnk'))
        $shortcut.TargetPath = Join-Path $root 'VaultAgent.exe'; $shortcut.Arguments = '--tray'; $shortcut.WorkingDirectory = $root
        $shortcut.IconLocation = (Join-Path $root 'VaultAgent.exe') + ',0'; $shortcut.Save()
    }
    @{ version='2.0.0'; service='HomelabVaultAgent'; installedUtc=[DateTime]::UtcNow.ToString('o'); packageVerified=$true } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'install-state.json') -Encoding utf8
    Start-Process -FilePath (Join-Path $root 'VaultAgent.exe') -ArgumentList @('--settings','--owner',$OwnerSid) | Out-Null
}
catch {
    Write-InstallStage 'failed' $_.Exception.Message
    if (Test-Path $root) {
        @{ status='failed'; date=[DateTime]::UtcNow.ToString('o'); error=$_.Exception.Message } | ConvertTo-Json |
            Set-Content -LiteralPath (Join-Path $root 'install-error.json') -Encoding utf8
    }
    Add-Type -AssemblyName System.Windows.Forms
    [Windows.Forms.MessageBox]::Show($_.Exception.Message,'Installation du coffre Bitwarden','OK','Error') | Out-Null
    exit 1
}
