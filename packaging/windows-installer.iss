; Hangar setup for Windows: per user, no admin rights. Built by windows-installer.ps1, which runs:
;   ISCC /DAppVersion=<version> /DExe=<path to hangar.exe> /O<output dir> packaging\windows-installer.iss

#ifndef AppVersion
  #error Pass /DAppVersion=<version>
#endif
#ifndef Exe
  #error Pass /DExe=<path to hangar.exe>
#endif

[Setup]
; Never change AppId: Windows uses it to recognise an installed Hangar for upgrades and uninstall.
AppId={{FA57B512-195F-48A5-8784-21324EA2F560}
AppName=Hangar
AppVersion={#AppVersion}
AppPublisher=Ellert van der Vecht
AppPublisherURL=https://github.com/evandervecht/hangar
AppSupportURL=https://github.com/evandervecht/hangar/issues
; The folder install.ps1 uses, so the setup installs over a source install and reuses its PATH entry.
DefaultDirName={localappdata}\Programs\hangar
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=hangar-{#AppVersion}-windows-amd64-setup
SetupIconFile=..\assets\hangar.ico
UninstallDisplayIcon={app}\hangar.exe
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
ChangesEnvironment=yes
; Closing the window only hides Hangar to the tray, so a polite close isn't enough.
CloseApplications=force
RestartApplications=no

[Tasks]
Name: path; Description: "Add hangar to PATH, for the command line"
Name: desktopicon; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked

[Files]
Source: "{#Exe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Hangar"; Filename: "{app}\hangar.exe"
Name: "{autodesktop}\Hangar"; Filename: "{app}\hangar.exe"; Tasks: desktopicon

[UninstallDelete]
; The folder may predate this setup (install.ps1); remove it too once it's empty.
Type: dirifempty; Name: "{app}"

[Run]
Filename: "{app}\hangar.exe"; Description: "Launch Hangar"; Flags: nowait postinstall skipifsilent

[Code]
const
  EnvKey = 'Environment';
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';

function UserPath(): string;
begin
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Result) then
    Result := '';
end;

// Position of Dir as a whole entry in ';' + path + ';' (case-insensitive), 0 if absent.
function EntryPos(Dir, Path: string): Integer;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Path) + ';');
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Path, Dir: string;
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('path') then
  begin
    Path := UserPath();
    Dir := ExpandConstant('{app}');
    if EntryPos(Dir, Path) = 0 then
    begin
      if (Path <> '') and (Path[Length(Path)] <> ';') then
        Path := Path + ';';
      // Expandable string, so existing %VAR% entries keep working.
      RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Path + Dir);
    end;
  end;
end;

// The uninstaller doesn't close running apps like setup does; a Hangar in the tray would keep its exe.
// Only the copy in this install folder is stopped, not a build running from elsewhere.
procedure StopHangar(Dir: string);
var
  Exe: string;
  Code: Integer;
begin
  Exe := Dir + '\hangar.exe';
  StringChangeEx(Exe, '''', '''''', True);
  Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
    '-NoProfile -NonInteractive -Command "Get-Process hangar -ErrorAction SilentlyContinue | ' +
    'Where-Object Path -eq ''' + Exe + ''' | Stop-Process -Force; Start-Sleep -Milliseconds 500"',
    '', SW_HIDE, ewWaitUntilTerminated, Code);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Path, Dir: string;
  I: Integer;
begin
  if CurUninstallStep <> usUninstall then
    exit;
  Dir := ExpandConstant('{app}');
  StopHangar(Dir);
  Path := ';' + UserPath() + ';';
  I := EntryPos(Dir, Copy(Path, 2, Length(Path) - 2));
  if I > 0 then
  begin
    Delete(Path, I, Length(Dir) + 1);
    RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Copy(Path, 2, Length(Path) - 2));
  end;
  // "Start at login" would otherwise point at a deleted exe.
  RegDeleteValue(HKCU, RunKey, 'Hangar');
  if not UninstallSilent and
     (MsgBox('Also delete your Hangar data (projects, tags, notes and settings)?' + #13#10#13#10 +
             'Choose No to keep it for a later install.', mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES) then
    DelTree(ExpandConstant('{userappdata}\hangar'), True, True, True);
end;
