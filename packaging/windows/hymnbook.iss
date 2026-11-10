; Inno Setup script: builds the Windows installer (no admin rights needed).
; Build:  iscc /DMyAppVersion=0.1.1 packaging\windows\hymnbook.iss  (run from the repo root)
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif

[Setup]
AppId={{8F3C2B1E-5A47-4C0D-9B7E-3D1F6A0C9E42}
AppName=Hymn Book
AppVersion={#MyAppVersion}
AppPublisher=Hymn Book contributors
AppPublisherURL=https://github.com/ugochukwu-850/hymnbook
DefaultDirName={autopf}\Hymn Book
DefaultGroupName=Hymn Book
UninstallDisplayIcon={app}\HymnBook.exe
SetupIconFile=..\..\assets\icon\icon.ico
OutputDir=..\..\dist
OutputBaseFilename=HymnBook-Setup-windows-x86_64
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\..\LICENSE

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"

[Files]
Source: "..\..\dist\HymnBook-windows-x86_64.exe"; DestDir: "{app}"; DestName: "HymnBook.exe"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\THIRD_PARTY.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Hymn Book"; Filename: "{app}\HymnBook.exe"
Name: "{autodesktop}\Hymn Book"; Filename: "{app}\HymnBook.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\HymnBook.exe"; Description: "Launch Hymn Book"; Flags: nowait postinstall skipifsilent
