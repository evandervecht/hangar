# Build Hangar and install it for the current user (Windows).
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

$bin = Join-Path $env:LOCALAPPDATA 'Programs\hangar'
New-Item -ItemType Directory -Force $bin | Out-Null
New-Item -ItemType Directory -Force (Join-Path $env:APPDATA 'hangar') | Out-Null
Copy-Item target\release\hangar.exe $bin -Force

# Start Menu shortcut so the window can be opened without a terminal.
$lnk = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Hangar.lnk'
$s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
$s.TargetPath = Join-Path $bin 'hangar.exe'
$s.Save()

Write-Host "Installed to $bin. Add it to PATH to use 'hangar scan' / 'hangar list'."
