; unNik Windows installer - Inno Setup 6.3 or newer (https://jrsoftware.org/isinfo.php)
; Installs unNik ALONE: the free browser and extractor for .nik archives (freeware,
; LICENSE.txt section 2a: it may be distributed on its own). This is the setup for
; people who only receive .nik archives. NikPak's own setup (nikpak.iss) already
; contains unNik.
; - Start menu: unNik.   Desktop: optional.
; - .nik files open in unNik (own NikPak-archive icon), right-click submenu "unNik":
;   Open with unNik / Extract files... / Extract here / Extract to its own folder.
;   When NikPak is already installed, NikPak's file type and "NikPak" submenu are kept
;   and this setup only installs the program (no second submenu).
;
; Built by:   .\buildall.ps1      (or .\build.ps1 -Installer for NikPak's setup only)
; which runs: ISCC /DMyAppVersion=<version from Cargo.toml> installer\unnik.iss
; Result:     dist\unnik-<version>-setup.exe
;
; Installs for the current user only by default (no administrator rights) into
; %LOCALAPPDATA%\Programs\unNik; the first page offers "install for all users".

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif

[Setup]
; Never change AppId: it lets updates replace the installed version.
AppId={{3C8E5B2A-9D41-4F6E-8B7A-1E2D5C9F0A63}
AppName=unNik
AppVersion={#MyAppVersion}
AppVerName=unNik {#MyAppVersion}
AppPublisher=Nikkoid
AppPublisherURL=https://nikpak.nikkoid.com
AppSupportURL=https://t.me/cryptonikko
AppUpdatesURL=https://nikpak.nikkoid.com
AppCopyright=(c) 2026 Nikkoid
VersionInfoVersion={#MyAppVersion}
VersionInfoDescription=unNik setup (free .nik archive browser)
DefaultDirName={autopf}\unNik
DefaultGroupName=unNik
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\LICENSE.txt
SetupIconFile=..\assets\unnik.ico
UninstallDisplayIcon={app}\unnik.ico
UninstallDisplayName=unNik
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir=..\dist
OutputBaseFilename=unnik-{#MyAppVersion}-setup
ChangesEnvironment=yes
ChangesAssociations=yes
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[CustomMessages]
english.MenuOpen=Open with unNik
english.MenuExtract=Extract files...
english.MenuHere=Extract here
english.MenuTo=Extract to its own folder
english.TypeName=NikPak archive
english.TaskMenu=Add unNik to the right-click menu of .nik archives (Extract here...)
english.TaskGroup=Explorer:
french.MenuOpen=Ouvrir avec unNik
french.MenuExtract=Extraire les fichiers...
french.MenuHere=Extraire ici
french.MenuTo=Extraire dans son propre dossier
french.TypeName=Archive NikPak
french.TaskMenu=Ajouter unNik au menu contextuel des archives .nik (Extraire ici...)
french.TaskGroup=Explorateur :

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "contextmenu"; Description: "{cm:TaskMenu}"; GroupDescription: "{cm:TaskGroup}"; Check: not NikPakInstalled
; Checked by default: `unNik` must work in any command prompt.
Name: "addtopath"; Description: "Add unNik to the PATH (use it from any command prompt)"; GroupDescription: "Command line:"

[Files]
Source: "..\dist\unNik.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\unnik.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\nikfile.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\unNik"; Filename: "{app}\unNik.exe"; IconFilename: "{app}\unnik.ico"; Comment: "Browse and extract .nik archives (free)"
Name: "{autodesktop}\unNik"; Filename: "{app}\unNik.exe"; IconFilename: "{app}\unnik.ico"; Comment: "Browse and extract .nik archives (free)"; Tasks: desktopicon

[Registry]
; .nik file type (own ProgID, so that NikPak's setup and this one never remove each other's).
; Skipped when NikPak is installed: NikPak already registers .nik files with unNik.
Root: HKA; Subkey: "Software\Classes\.nik"; ValueType: string; ValueName: ""; ValueData: "unNik.Archive"; Flags: uninsdeletevalue; Check: not NikPakInstalled
Root: HKA; Subkey: "Software\Classes\.nik"; ValueType: string; ValueName: "Content Type"; ValueData: "application/x-nikpak"; Check: not NikPakInstalled
Root: HKA; Subkey: "Software\Classes\.nik"; ValueType: string; ValueName: "PerceivedType"; ValueData: "compressed"; Check: not NikPakInstalled
Root: HKA; Subkey: "Software\Classes\.nik\OpenWithProgids"; ValueType: string; ValueName: "unNik.Archive"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\unNik.Archive"; ValueType: string; ValueName: ""; ValueData: "{cm:TypeName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\unNik.Archive"; ValueType: string; ValueName: "FriendlyTypeName"; ValueData: "{cm:TypeName}"
Root: HKA; Subkey: "Software\Classes\unNik.Archive\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\nikfile.ico,0"
Root: HKA; Subkey: "Software\Classes\unNik.Archive\shell"; ValueType: string; ValueName: ""; ValueData: "open"
Root: HKA; Subkey: "Software\Classes\unNik.Archive\shell\open"; ValueType: string; ValueName: ""; ValueData: "{cm:MenuOpen}"
Root: HKA; Subkey: "Software\Classes\unNik.Archive\shell\open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\unNik.exe,0"
Root: HKA; Subkey: "Software\Classes\unNik.Archive\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\unNik.exe"" ""%1"""
; "Open with" list
Root: HKA; Subkey: "Software\Classes\Applications\unNik.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "unNik"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\unNik.exe\SupportedTypes"; ValueType: string; ValueName: ".nik"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\unNik.exe\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\unNik.exe"" ""%1"""

; Right-click submenu "unNik" on .nik archives (Windows 11: under "Show more options").
; Several selected archives are extracted one after the other in one window.
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik"; ValueType: string; ValueName: "MUIVerb"; ValueData: "unNik"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\unNik.exe,0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik"; ValueType: string; ValueName: "SubCommands"; ValueData: ""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\01open"; ValueType: string; ValueName: "MUIVerb"; ValueData: "{cm:MenuOpen}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\01open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\unNik.exe,0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\01open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\unNik.exe"" ""%1"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\02extract"; ValueType: string; ValueName: "MUIVerb"; ValueData: "{cm:MenuExtract}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\02extract"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\unNik.exe,0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\02extract"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\02extract\command"; ValueType: string; ValueName: ""; ValueData: """{app}\unNik.exe"" --extract ""%1"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\03here"; ValueType: string; ValueName: "MUIVerb"; ValueData: "{cm:MenuHere}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\03here"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\unNik.exe,0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\03here"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\03here\command"; ValueType: string; ValueName: ""; ValueData: """{app}\unNik.exe"" --extract-here ""%1"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\04to"; ValueType: string; ValueName: "MUIVerb"; ValueData: "{cm:MenuTo}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\04to"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\unNik.exe,0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\04to"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik\shell\04to\command"; ValueType: string; ValueName: ""; ValueData: """{app}\unNik.exe"" --extract-to ""%1"""; Tasks: contextmenu
; the uninstaller always removes the submenu, even if the option was changed later
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\unNik"; Flags: uninsdeletekey dontcreatekey

[Run]
Filename: "{app}\unNik.exe"; Description: "{cm:LaunchProgram,unNik}"; Flags: nowait postinstall skipifsilent runasoriginaluser

[Code]
const
  NikPakUninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{75265F6E-3E8B-4CA3-BC75-5349E6A2F55A}_is1';
  EnvKeyUser = 'Environment';
  EnvKeyMachine = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';

{ NikPak's setup (which contains unNik and its own .nik menu) is installed. }
function NikPakInstalled: Boolean;
begin
  Result := RegKeyExists(HKEY_CURRENT_USER, NikPakUninstallKey)
    or RegKeyExists(HKLM64, NikPakUninstallKey)
    or RegKeyExists(HKLM32, NikPakUninstallKey);
end;

{ ---- PATH: `unNik` (and nikpak, in the same folder) in any command prompt ---- }
function EnvRoot: Integer;
begin
  if IsAdminInstallMode then Result := HKEY_LOCAL_MACHINE else Result := HKEY_CURRENT_USER;
end;

function EnvKey: String;
begin
  if IsAdminInstallMode then Result := EnvKeyMachine else Result := EnvKeyUser;
end;

procedure AddToPath(Dir: String);
var
  P: String;
begin
  if not RegQueryStringValue(EnvRoot, EnvKey, 'Path', P) then P := '';
  if Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(P) + ';') > 0 then exit;
  if (P <> '') and (P[Length(P)] <> ';') then P := P + ';';
  P := P + Dir;
  RegWriteExpandStringValue(EnvRoot, EnvKey, 'Path', P);
end;

procedure RemoveFromPath(Dir: String);
var
  P, D: String;
  I: Integer;
begin
  if not RegQueryStringValue(EnvRoot, EnvKey, 'Path', P) then exit;
  P := ';' + P + ';';
  D := ';' + Dir + ';';
  I := Pos(Uppercase(D), Uppercase(P));
  if I = 0 then exit;
  Delete(P, I, Length(D) - 1);
  P := Copy(P, 2, Length(P) - 2);
  RegWriteExpandStringValue(EnvRoot, EnvKey, 'Path', P);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;
