; Inno Setup script for the Windows installer.
; Built by scripts/package-windows.ps1:  iscc /DVersion=0.1.0 packaging\windows\bloom-and-gather.iss
#ifndef Version
  #define Version "0.0.0"
#endif

[Setup]
AppId={{6B0E2F57-9C41-4D8A-A3E2-B10057A7E2F1}
AppName=Bloom & Gather
AppVersion={#Version}
AppPublisher=Bloom & Gather
DefaultDirName={autopf}\Bloom & Gather
DefaultGroupName=Bloom & Gather
DisableProgramGroupPage=yes
; Installs for the current user unless the player chooses all users.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\..\dist
OutputBaseFilename=Bloom-and-Gather-{#Version}-windows-x86_64-setup
SetupIconFile=..\icon\icon.ico
UninstallDisplayIcon={app}\bloom-and-gather.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\target\dist\bloom-and-gather.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\icon\icon.ico"; DestDir: "{app}"; DestName: "bloom-and-gather.ico"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Icons]
Name: "{group}\Bloom & Gather"; Filename: "{app}\bloom-and-gather.exe"; IconFilename: "{app}\bloom-and-gather.ico"
Name: "{autodesktop}\Bloom & Gather"; Filename: "{app}\bloom-and-gather.exe"; IconFilename: "{app}\bloom-and-gather.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\bloom-and-gather.exe"; Description: "{cm:LaunchProgram,Bloom & Gather}"; Flags: nowait postinstall skipifsilent
