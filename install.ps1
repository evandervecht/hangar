# Build Hangar and install it for the current user (Windows).
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
cargo build --release --locked
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

# Add to the user PATH via the registry: keeps it REG_EXPAND_SZ so existing %VAR% entries survive.
$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
$old = [string]$key.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
if (($old -split ';') -notcontains $bin) {
    $key.SetValue('Path', (@($old.TrimEnd(';'), $bin) -join ';').TrimStart(';'), 'ExpandString')
    # Setting any user variable broadcasts the change, so new terminals see the new PATH.
    [Environment]::SetEnvironmentVariable('HANGAR_INSTALL', $null, 'User')
    Write-Host "Added $bin to your PATH."
}
$key.Close()

Write-Host "Installed to $bin. Open a new terminal and run 'hangar' (window), 'hangar scan' or 'hangar list'."
