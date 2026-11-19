; Duck Mold's Windows installer: DuckMoldSetup-<version>.exe.
;
; Made with Inno Setup 6 (free, https://jrsoftware.org/isinfo.php) by the
; release workflow (.github/workflows/release.yml), from the same folder
; the zip is made from:
;
;   ISCC.exe /DVersion=1.0.0 /DSource="dist\Duck Mold" /O"dist" release\DuckMold.iss
;
; Copied from Duck Forge Studio's installer, and changed. What it does:
; - Installs for you alone, with no administrator rights, in
;   %LOCALAPPDATA%\Programs\Duck Mold (a folder you can write to).
; - Adds Duck Mold Studio to the Start menu, offers a desktop shortcut,
;   and lists Duck Mold in Windows Settings > Apps, to uninstall it.
; - Installs the examples and the guides beside the programs.
; - A ticked box adds the folder to your PATH, so that a command line or
;   an AI agent finds duck-mold. Uninstalling takes it away again.
; - Makes no file association: .toml files are used by many programs.
; - Never touches your models. They live in Documents\Duck Mold, never in
;   the program's folder; installing a newer version over this one, or
;   uninstalling, leaves them as they are.

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Source
  #define Source "..\dist\Duck Mold"
#endif
#ifndef SetupName
  #define SetupName "DuckMoldSetup-" + Version
#endif

[Setup]
; The same id in every version, so a newer installer replaces an older
; install instead of adding a second one. Never change it.
AppId={{6F2C1D84-3B7E-4E8A-9C51-2D7A0B9E4F63}
AppName=Duck Mold
AppVersion={#Version}
AppVerName=Duck Mold {#Version}
AppPublisher=Duck Mold
AppPublisherURL=https://github.com/mrguymrguy647-del/Duck-Mold
AppSupportURL=https://github.com/mrguymrguy647-del/Duck-Mold
AppUpdatesURL=https://github.com/mrguymrguy647-del/Duck-Mold/releases
VersionInfoVersion={#Version}
; Just for you: no administrator rights, and {autopf} is then
; %LOCALAPPDATA%\Programs, which you can write to.
PrivilegesRequired=lowest
DefaultDirName={autopf}\Duck Mold
DisableDirPage=yes
DisableProgramGroupPage=yes
OutputBaseFilename={#SetupName}
SetupIconFile=..\crates\duck-mold-studio\assets\icon.ico
UninstallDisplayIcon={app}\DuckMoldStudio.exe
UninstallDisplayName=Duck Mold
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; If the Studio is open, close it first.
CloseApplications=yes
RestartApplications=no
; The PATH changes: tell open programs.
ChangesEnvironment=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Put a Duck Mold Studio icon on the desktop"; GroupDescription: "Icons:"; Flags: unchecked
Name: "addtopath"; Description: "Let the command line and AI agents find duck-mold"; GroupDescription: "The command line:"

[InstallDelete]
; The examples and guides that came with an older version are replaced
; whole, so no old file is left. Nothing of yours is ever in here.
Type: filesandordirs; Name: "{app}\examples"
Type: filesandordirs; Name: "{app}\docs"

[Files]
; Everything in the zip, except the zip's shortcut maker: the installer
; makes the shortcuts itself.
Source: "{#Source}\*"; DestDir: "{app}"; Excludes: "Create desktop shortcut.bat"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Duck Mold Studio"; Filename: "{app}\DuckMoldStudio.exe"; WorkingDir: "{app}"; Comment: "Build 3D models from parts, for Duck Forge games"
Name: "{autodesktop}\Duck Mold Studio"; Filename: "{app}\DuckMoldStudio.exe"; WorkingDir: "{app}"; Comment: "Build 3D models from parts, for Duck Forge games"; Tasks: desktopicon

[Registry]
; Your own PATH (not every user's), with the program's folder added once.
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Tasks: addtopath; Check: NotOnPath(ExpandConstant('{app}'))

[Run]
Filename: "{app}\DuckMoldStudio.exe"; Description: "Start Duck Mold Studio now"; Flags: nowait postinstall skipifsilent

[Code]
{ The PATH as it is kept for you, in the registry. }
function OwnPath(): String;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Result) then
    Result := '';
end;

{ True if Folder isn't one of the PATH's entries yet. }
function NotOnPath(Folder: String): Boolean;
begin
  Result := Pos(';' + Uppercase(Folder) + ';', ';' + Uppercase(OwnPath()) + ';') = 0;
end;

{ Uninstalling takes the folder out of the PATH again, and nothing else. }
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Path, Folder: String;
  At: Integer;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    Path := ';' + OwnPath() + ';';
    Folder := ';' + ExpandConstant('{app}') + ';';
    At := Pos(Uppercase(Folder), Uppercase(Path));
    if At > 0 then
    begin
      Delete(Path, At, Length(Folder) - 1);
      { Without the ; added at each end. }
      Path := Copy(Path, 2, Length(Path) - 2);
      RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path);
    end;
  end;
end;
