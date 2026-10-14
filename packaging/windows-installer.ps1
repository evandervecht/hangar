# Build hangar-<version>-windows-amd64-setup.exe with Inno Setup 6 (Windows only).
# Usage: packaging/windows-installer.ps1 <hangar.exe> <version> <dest-dir>
param([Parameter(Mandatory)] $Exe, [Parameter(Mandatory)] $Version, [Parameter(Mandatory)] $Dest)
$ErrorActionPreference = 'Stop'

# GitHub's Windows runners have it in Program Files (x86); winget --scope user puts it in LocalAppData.
$iscc = @(
    (Get-Command iscc -ErrorAction SilentlyContinue).Source
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) { throw 'Inno Setup 6 not found: winget install JRSoftware.InnoSetup' }

New-Item -ItemType Directory -Force $Dest | Out-Null
& $iscc /Q "/DAppVersion=$Version" "/DExe=$((Resolve-Path $Exe).Path)" "/O$((Resolve-Path $Dest).Path)" "$PSScriptRoot\windows-installer.iss"
if ($LASTEXITCODE -ne 0) { throw "ISCC failed with exit code $LASTEXITCODE" }
