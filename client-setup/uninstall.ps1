<# Retirer le programme de ce compte ; conserver l'association DPAPI pour une reinstallation. #>
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
if([Windows.Forms.MessageBox]::Show('Desinstaller VaultAgent Client ? L association est conservee. Pour retirer son acces, revoquez le client sur le serveur.','VaultAgent Client','YesNo','Question') -ne 'Yes'){exit 0}
$expected=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs\VaultAgentClient'
$target=[IO.Path]::GetFullPath($PSScriptRoot)
if($target -ne [IO.Path]::GetFullPath($expected) -or [IO.Path]::GetFileName($target) -ne 'VaultAgentClient'){throw 'Chemin de desinstallation refuse.'}
if((Get-Item -LiteralPath $target).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Dossier lie refuse.'}
foreach($file in Get-ChildItem -LiteralPath $target -Recurse -Force){if($file.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Lien refuse.'}}
$executable=Join-Path $target 'VaultAgent-Client.exe'
Get-Process -Name VaultAgent-Client -ErrorAction SilentlyContinue | ForEach-Object {if($_.Path -eq $executable){Stop-Process -Id $_.Id -Force}}
$run='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$value=(Get-ItemProperty -Path $run -Name VaultAgentClient -ErrorAction SilentlyContinue).VaultAgentClient
if($value -and $value.Contains($executable)){Remove-ItemProperty -Path $run -Name VaultAgentClient}
$shell=New-Object -ComObject WScript.Shell
foreach($folder in @([Environment]::GetFolderPath('DesktopDirectory'),[Environment]::GetFolderPath('Programs'))){
  $link=Join-Path $folder 'VaultAgent Client.lnk'
  if((Test-Path -LiteralPath $link) -and $shell.CreateShortcut($link).TargetPath -eq $executable){Remove-Item -LiteralPath $link}
}
Remove-Item -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\VaultAgentClient' -Recurse -Force -ErrorAction SilentlyContinue
if([IO.Path]::GetFullPath($target) -ne [IO.Path]::GetFullPath($expected)){throw 'Chemin final refuse.'}
Remove-Item -LiteralPath $target -Recurse -Force
