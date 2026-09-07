# Zegion installer for Windows.
# Usage: irm https://raw.githubusercontent.com/zakirkun/zegion/main/install.ps1 | iex
$ErrorActionPreference = 'Stop'

$Repo = 'zakirkun/zegion'
$InstallDir = if ($env:ZEGION_INSTALL_DIR) { $env:ZEGION_INSTALL_DIR } else { "$env:LOCALAPPDATA\Programs\zegion" }

function Get-LatestVersion {
    $rel = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ 'User-Agent' = 'zegion-installer' }
    return $rel.tag_name
}

$version = if ($env:ZEGION_VERSION) { $env:ZEGION_VERSION } else { Get-LatestVersion }
if (-not $version) { throw 'Could not determine latest release.' }

$url = "https://github.com/$Repo/releases/download/$version/zegion-windows-x86_64.zip"
Write-Host "Installing Zegion $version -> $InstallDir"

$tmp = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ([Guid]::NewGuid().ToString()))
try {
    $zip = Join-Path $tmp.FullName 'zegion.zip'
    Invoke-WebRequest -Uri $url -OutFile $zip -Headers @{ 'User-Agent' = 'zegion-installer' }
    Expand-Archive -Path $zip -DestinationPath $tmp.FullName -Force

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item (Join-Path $tmp.FullName 'zegion.exe') (Join-Path $InstallDir 'zegion.exe') -Force

    # Add to user PATH if missing.
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$InstallDir", 'User')
        Write-Host "Added $InstallDir to your PATH (restart your terminal)."
    }

    Write-Host "Installed: $InstallDir\zegion.exe"
    Write-Host "Run 'zegion onboard' to configure, then 'zegion run'."
}
finally {
    Remove-Item -Recurse -Force $tmp.FullName -ErrorAction SilentlyContinue
}
