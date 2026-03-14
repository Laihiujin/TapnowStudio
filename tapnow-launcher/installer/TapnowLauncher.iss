#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

[Setup]
AppId={{7ACBC1BC-A593-43F4-8D6D-F4B6E6A808F1}
AppName=TapnowStudio
AppVersion={#MyAppVersion}
AppPublisher=TapnowStudio
DefaultDirName={localappdata}\TapnowStudio
DefaultGroupName=TapnowStudio
DisableProgramGroupPage=yes
OutputBaseFilename=TapnowStudio_Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\TapnowStudio.exe
SetupLogging=yes
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "chinesesimp"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create desktop shortcut"; GroupDescription: "Additional tasks:"; Flags: unchecked

[Files]
Source: "..\build\bundle\TapnowStudio.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\build\bundle\README.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\build\bundle\runtime\*"; DestDir: "{app}\runtime"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\TapnowStudio\TapnowStudio"; Filename: "{app}\TapnowStudio.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\TapnowStudio"; Filename: "{app}\TapnowStudio.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\TapnowStudio.exe"; Description: "Launch TapnowStudio now"; Flags: nowait postinstall skipifsilent

[Code]
function InitializeSetup(): Boolean;
var
  ResultCode: Integer;
begin
  Exec(
    ExpandConstant('{cmd}'),
    '/C taskkill /F /IM TapnowStudio.exe >nul 2>nul',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  );
  Sleep(800);
  Result := True;
end;
