; NikPak Windows installer - Inno Setup 6.3 or newer (https://jrsoftware.org/isinfo.php)
; Installs NikPak (nikpak.exe, shareware) and unNik (unNik.exe, the free archive browser).
; - Start menu: NikPak and unNik.   Desktop: NikPak (always), unNik (optional).
; - .nik files are registered: they show the NikPak archive icon (nikfile.ico, like the zipper
;   icon of .zip files) and a double-click opens them in unNik. Right-click menu: Open with unNik.
; - Right-click menu "NikPak" (like WinRAR's), an option checked by default: files and folders get
;   "Add to archive..." and "Add to archive here", .nik archives get "Open with unNik",
;   "Extract files...", "Extract here" and "Extract to its own folder". The setup runs
;   nikpak.exe --install-menu (labels in the user's language); on Windows 11 the submenu is
;   under "Show more options".
; - Everything is removed again by the uninstaller.
;
; Built by:   .\build.ps1 -Installer      (or: npm run installer)
; which runs: ISCC /DMyAppVersion=<version from Cargo.toml> installer\nikpak.iss
; Result:     dist\nikpak-<version>-setup.exe   - the file to upload to shareware sites.
;
; By default NikPak installs for the current user only (no administrator rights, no UAC
; prompt) into %LOCALAPPDATA%\Programs\NikPak. The first page lets the user choose
; "install for all users" instead (Program Files, needs administrator rights).
; The licence (registration) is stored per user in %APPDATA%\nikpak and is kept when
; NikPak is uninstalled or updated.

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif

[Setup]
; Never change AppId: it lets updates replace the installed version.
AppId={{75265F6E-3E8B-4CA3-BC75-5349E6A2F55A}
AppName=NikPak
AppVersion={#MyAppVersion}
AppVerName=NikPak {#MyAppVersion}
AppPublisher=Nikkoid
AppPublisherURL=https://nikpak.nikkoid.com
AppSupportURL=https://t.me/cryptonikko
AppUpdatesURL=https://nikpak.nikkoid.com
AppCopyright=(c) 2026 Nikkoid
VersionInfoVersion={#MyAppVersion}
VersionInfoDescription=NikPak setup
DefaultDirName={autopf}\NikPak
DefaultGroupName=NikPak
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\LICENSE.txt
SetupIconFile=..\assets\nikpak.ico
UninstallDisplayIcon={app}\nikpak.ico
UninstallDisplayName=NikPak
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir=..\dist
OutputBaseFilename=nikpak-{#MyAppVersion}-setup
ChangesEnvironment=yes
ChangesAssociations=yes
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "unnikdesktop"; Description: "Also put unNik (the archive browser) on the desktop"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "contextmenu"; Description: "Add NikPak to the right-click menu of Explorer (Add to archive, Extract here...)"; GroupDescription: "Explorer:"
; Checked by default: `nikpak` and `unNik` must work in any command prompt.
; ChangesEnvironment=yes (above) makes Setup announce the new PATH to Windows.
Name: "addtopath"; Description: "Add nikpak to the PATH (use it from any command prompt)"; GroupDescription: "Command line:"

[Files]
Source: "..\dist\nikpak.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\unNik.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\nikpak.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\unnik.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\nikfile.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
; Never add keys\, nikpak.key or license.txt here: the setup is distributed publicly.

[Icons]
Name: "{autoprograms}\NikPak"; Filename: "{app}\nikpak.exe"; IconFilename: "{app}\nikpak.ico"; Comment: "Compress files and folders into .nik archives"
Name: "{autodesktop}\NikPak"; Filename: "{app}\nikpak.exe"; IconFilename: "{app}\nikpak.ico"; Comment: "Compress files and folders into .nik archives"
Name: "{autoprograms}\unNik"; Filename: "{app}\unNik.exe"; IconFilename: "{app}\unnik.ico"; Comment: "Browse and extract .nik archives (free)"
Name: "{autodesktop}\unNik"; Filename: "{app}\unNik.exe"; IconFilename: "{app}\unnik.ico"; Comment: "Browse and extract .nik archives (free)"; Tasks: unnikdesktop
Name: "{autoprograms}\NikPak licence"; Filename: "{app}\LICENSE.txt"; Comment: "Licence terms of NikPak and unNik"

[Registry]
; .nik archives. HKA = per user (HKCU\Software\Classes) or all users (HKLM), following the install mode.
; The file type gets its own icon (nikfile.ico) and opens in unNik, like .zip files in Explorer.
Root: HKA; Subkey: "Software\Classes\.nik"; ValueType: string; ValueName: ""; ValueData: "NikPak.Archive"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.nik"; ValueType: string; ValueName: "Content Type"; ValueData: "application/x-nikpak"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.nik"; ValueType: string; ValueName: "PerceivedType"; ValueData: "compressed"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.nik\OpenWithProgids"; ValueType: string; ValueName: "NikPak.Archive"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\NikPak.Archive"; ValueType: string; ValueName: ""; ValueData: "NikPak archive"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\NikPak.Archive"; ValueType: string; ValueName: "FriendlyTypeName"; ValueData: "NikPak archive"
Root: HKA; Subkey: "Software\Classes\NikPak.Archive\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\nikfile.ico,0"
Root: HKA; Subkey: "Software\Classes\NikPak.Archive\shell"; ValueType: string; ValueName: ""; ValueData: "open"
Root: HKA; Subkey: "Software\Classes\NikPak.Archive\shell\open"; ValueType: string; ValueName: ""; ValueData: "Open with unNik"
Root: HKA; Subkey: "Software\Classes\NikPak.Archive\shell\open"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "unNik"
Root: HKA; Subkey: "Software\Classes\NikPak.Archive\shell\open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\unnik.ico"
Root: HKA; Subkey: "Software\Classes\NikPak.Archive\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\unNik.exe"" ""%1"""
; "Open with" list and default-apps settings
Root: HKA; Subkey: "Software\Classes\Applications\unNik.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "unNik"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\unNik.exe\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\unnik.ico"
Root: HKA; Subkey: "Software\Classes\Applications\unNik.exe\SupportedTypes"; ValueType: string; ValueName: ".nik"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\unNik.exe\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\unNik.exe"" ""%1"""
Root: HKA; Subkey: "Software\NikPak\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "unNik"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\NikPak\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Browse and extract .nik archives"
Root: HKA; Subkey: "Software\NikPak\Capabilities\FileAssociations"; ValueType: string; ValueName: ".nik"; ValueData: "NikPak.Archive"
Root: HKA; Subkey: "Software\NikPak"; Flags: uninsdeletekeyifempty
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "NikPak"; ValueData: "Software\NikPak\Capabilities"; Flags: uninsdeletevalue
; Right-click menu: written by nikpak.exe --install-menu ([Run] below). These lines only make
; sure the uninstaller removes it even if nikpak.exe could not run.
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\NikPak"; Flags: uninsdeletekey dontcreatekey
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.nik\shell\NikPak"; Flags: uninsdeletekey dontcreatekey

[Run]
; right-click menu: added when the option is checked, removed when it is not (update without it)
Filename: "{app}\nikpak.exe"; Parameters: "--install-menu"; Flags: runhidden waituntilterminated; Tasks: contextmenu; Check: not IsAdminInstallMode
Filename: "{app}\nikpak.exe"; Parameters: "--install-menu --all-users"; Flags: runhidden waituntilterminated; Tasks: contextmenu; Check: IsAdminInstallMode
Filename: "{app}\nikpak.exe"; Parameters: "--uninstall-menu"; Flags: runhidden waituntilterminated; Tasks: not contextmenu; Check: not IsAdminInstallMode
Filename: "{app}\nikpak.exe"; Parameters: "--uninstall-menu --all-users"; Flags: runhidden waituntilterminated; Tasks: not contextmenu; Check: IsAdminInstallMode
; runasoriginaluser: never start NikPak as administrator (that would block drag and drop)
Filename: "{app}\nikpak.exe"; Description: "{cm:LaunchProgram,NikPak}"; Flags: nowait postinstall skipifsilent runasoriginaluser

[UninstallRun]
Filename: "{app}\nikpak.exe"; Parameters: "--uninstall-menu"; Flags: runhidden waituntilterminated; RunOnceId: "NikPakMenuUser"; Check: not IsAdminInstallMode
Filename: "{app}\nikpak.exe"; Parameters: "--uninstall-menu --all-users"; Flags: runhidden waituntilterminated; RunOnceId: "NikPakMenuAll"; Check: IsAdminInstallMode

[Code]
const
  EnvKeyUser = 'Environment';
  EnvKeyMachine = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';

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
