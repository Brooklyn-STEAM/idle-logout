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
; Older versions started from the Run key; the service does that now.
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Idle Logout"; Flags: deletevalue
; Settings, in seconds; read at startup. Created with the defaults so admins can find them, and left alone on upgrade.
Root: HKLM; Subkey: "Software\Idle Logout"; ValueType: dword; ValueName: "IdleSeconds"; ValueData: "60"; Flags: createvalueifdoesntexist uninsdeletekey
Root: HKLM; Subkey: "Software\Idle Logout"; ValueType: dword; ValueName: "CountdownSeconds"; ValueData: "240"; Flags: createvalueifdoesntexist uninsdeletekey

[Run]
; The service runs as SYSTEM, so users can't stop it, and starts the app in every signed-in session that lacks it.
; "create" fails harmlessly on upgrade, when the service already exists.
Filename: "{sys}\sc.exe"; Parameters: "create IdleLogout binPath= ""\""{app}\idle-logout.exe\"" --service"" start= auto DisplayName= ""Idle Logout"""; Flags: runhidden
Filename: "{sys}\sc.exe"; Parameters: "failure IdleLogout reset= 0 actions= restart/5000"; Flags: runhidden
Filename: "{sys}\sc.exe"; Parameters: "start IdleLogout"; Flags: runhidden

[UninstallRun]
Filename: "{sys}\sc.exe"; Parameters: "stop IdleLogout"; Flags: runhidden waituntilterminated; RunOnceId: "StopService"
Filename: "{sys}\sc.exe"; Parameters: "delete IdleLogout"; Flags: runhidden; RunOnceId: "DeleteService"
Filename: "taskkill.exe"; Parameters: "/f /im idle-logout.exe"; Flags: runhidden; RunOnceId: "StopIdleLogout"

[Code]
// Upgrades: stop the service first, or it relaunches the app and the exe stays locked.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\sc.exe'), 'stop IdleLogout', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/f /im idle-logout.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Result := '';
end;
