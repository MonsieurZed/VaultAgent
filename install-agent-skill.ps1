<# Installer/mettre a jour le skill pour Codex et Claude Code sans ecraser les modifications locales. #>
[CmdletBinding(SupportsShouldProcess)]
param([ValidateSet('Codex','Claude','Both')][string]$Agent = 'Both', [switch]$Uninstall, [switch]$Update)
$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot 'skills\vault-agent'
$profileRoot = [Environment]::GetFolderPath('UserProfile')
$destinations = @()
if ($Agent -in @('Codex','Both')) { $destinations += Join-Path $profileRoot '.agents\skills\vault-agent' }
if ($Agent -in @('Claude','Both')) { $destinations += Join-Path $profileRoot '.claude\skills\vault-agent' }
if ($Uninstall -and $Update) { throw 'Choisir Update ou Uninstall.' }
if (-not $Uninstall -and -not $Update) {
    foreach ($destination in $destinations) {
        if (Test-Path -LiteralPath $destination) { throw "Un skill existe deja : $destination. Ne pas ecraser les modifications locales." }
    }
}
foreach ($destination in $destinations) {
    $root = [IO.Path]::GetFullPath($destination)
    if (-not $root.StartsWith($profileRoot + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Destination hors du profil utilisateur.' }
    $directory = [IO.DirectoryInfo]::new($root)
    while ($directory -and $directory.FullName -ne $profileRoot) {
        if ($directory.Exists -and ($directory.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Dossier lie refuse.' }
        $directory = $directory.Parent
    }
    $marker = Join-Path $root '.vault-agent-install.json'
    if ($Update -and (Test-Path -LiteralPath $root)) {
        if (-not (Test-Path -LiteralPath $marker)) { throw "Skill non gere : $root. Mise a jour refusee." }
        if (Get-ChildItem -LiteralPath $root -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint } | Select-Object -First 1) { throw 'Lien dans le skill : mise a jour refusee.' }
        $previous = Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json
        foreach ($entry in $previous.PSObject.Properties) {
            $target = [IO.Path]::GetFullPath((Join-Path $root $entry.Name))
            if (-not $target.StartsWith($root + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Chemin du manifeste refuse.' }
            if (-not (Test-Path -LiteralPath $target) -or (Get-FileHash -LiteralPath $target).Hash -ne $entry.Value) { throw "Modification locale conservee : $target. Mise a jour refusee." }
        }
        foreach ($file in Get-ChildItem -LiteralPath $source -File -Recurse) {
            $relative = $file.FullName.Substring($source.Length + 1)
            if ((Test-Path -LiteralPath (Join-Path $root $relative)) -and -not $previous.PSObject.Properties[$relative]) { throw "Fichier non gere conserve : $relative. Mise a jour refusee." }
        }
    }
    if ($Uninstall) {
        if (-not (Test-Path -LiteralPath $marker)) { Write-Warning "Skill non gere par ce script : $root"; continue }
        if (Get-ChildItem -LiteralPath $root -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint } | Select-Object -First 1) { throw 'Lien dans le skill : retrait automatique refuse.' }
        $manifest = Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json
        if ($PSCmdlet.ShouldProcess($root,'Retirer les fichiers du skill inchanges')) {
            foreach ($entry in $manifest.PSObject.Properties) {
                $target = [IO.Path]::GetFullPath((Join-Path $root $entry.Name))
                if (-not $target.StartsWith($root + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Chemin du manifeste refuse.' }
                if ((Test-Path -LiteralPath $target) -and (Get-FileHash -LiteralPath $target).Hash -eq $entry.Value) { Remove-Item -LiteralPath $target }
            }
            Remove-Item -LiteralPath $marker
            Get-ChildItem -LiteralPath $root -Directory -Recurse | Sort-Object { $_.FullName.Length } -Descending | ForEach-Object {
                if (-not (Get-ChildItem -LiteralPath $_.FullName -Force | Select-Object -First 1)) { Remove-Item -LiteralPath $_.FullName }
            }
            if (-not (Get-ChildItem -LiteralPath $root -Force | Select-Object -First 1)) { Remove-Item -LiteralPath $root }
        }
        continue
    }
    if ((Test-Path -LiteralPath $root) -and -not $Update) { throw "Un skill existe deja : $root. Ne pas ecraser les modifications locales." }
    if ($PSCmdlet.ShouldProcess($root,'Installer le skill VaultAgent')) {
        New-Item -ItemType Directory -Path $root -Force | Out-Null
        $manifest = @{}
        Get-ChildItem -LiteralPath $source -File -Recurse | ForEach-Object {
            $relative = $_.FullName.Substring($source.Length + 1)
            $target = Join-Path $root $relative
            New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null
            Copy-Item -LiteralPath $_.FullName -Destination $target
            $manifest[$relative] = (Get-FileHash -LiteralPath $target).Hash
        }
        $manifest | ConvertTo-Json | Set-Content -LiteralPath $marker -Encoding utf8
        Write-Output "Skill installe : $root"
    }
}
