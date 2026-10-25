; Inno Setup script. The release workflow builds it with:
;   iscc /DAppVersion=<version> installer.iss
; and expects idle-logout-x64.exe and idle-logout-arm64.exe next to this file.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{70fcc3da-189c-4ec4-b3b3-58fc10fb4519}
AppName=Idle Logout
AppVersion={#AppVersion}
AppPublisher=Brooklyn STEAM Center
DefaultDirName={autopf}\Idle Logout
DisableProgramGroupPage=yes
DisableDirPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible or arm64
ArchitecturesInstallIn64BitMode=x64compatible or arm64
OutputDir=.
OutputBaseFilename=idle-logout-setup
UninstallDisplayIcon={app}\idle-logout.exe

[Files]
Source: "idle-logout-arm64.exe"; DestDir: "{app}"; DestName: "idle-logout.exe"; Check: IsArm64; Flags: ignoreversion
Source: "idle-logout-x64.exe"; DestDir: "{app}"; DestName: "idle-logout.exe"; Check: not IsArm64; Flags: ignoreversion

[Registry]
; Starts for every user at sign-in.
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Idle Logout"; ValueData: """{app}\idle-logout.exe"""; Flags: uninsdeletevalue
; Settings, in seconds; read at startup. Created with the defaults so admins can find them, and left alone on upgrade.
Root: HKLM; Subkey: "Software\Idle Logout"; ValueType: dword; ValueName: "IdleSeconds"; ValueData: "60"; Flags: createvalueifdoesntexist uninsdeletekey
Root: HKLM; Subkey: "Software\Idle Logout"; ValueType: dword; ValueName: "CountdownSeconds"; ValueData: "240"; Flags: createvalueifdoesntexist uninsdeletekey

[Run]
Filename: "{app}\idle-logout.exe"; Flags: nowait postinstall skipifsilent runasoriginaluser; Description: "Start Idle Logout now"

[UninstallRun]
Filename: "taskkill.exe"; Parameters: "/f /im idle-logout.exe"; Flags: runhidden; RunOnceId: "StopIdleLogout"
