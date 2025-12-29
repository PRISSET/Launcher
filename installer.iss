[Setup]
AppName=ByStep Launcher
AppVersion=1.1.1
AppPublisher=ByStep
DefaultDirName={autopf}\ByStep Launcher
DefaultGroupName=ByStep Launcher
OutputDir=Output
OutputBaseFilename=ByStep-Launcher-Setup
Compression=lzma2
SolidCompression=yes
SetupIconFile=icon.ico
UninstallDisplayIcon={app}\ByStep-Launcher.exe
PrivilegesRequired=lowest
WizardStyle=modern

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "target\release\minecraft-launcher-native.exe"; DestDir: "{app}"; DestName: "ByStep-Launcher.exe"; Flags: ignoreversion

[Icons]
Name: "{group}\ByStep Launcher"; Filename: "{app}\ByStep-Launcher.exe"
Name: "{autodesktop}\ByStep Launcher"; Filename: "{app}\ByStep-Launcher.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Создать ярлык на рабочем столе"; GroupDescription: "Дополнительные значки:"

[Run]
Filename: "{app}\ByStep-Launcher.exe"; Description: "Запустить ByStep Launcher"; Flags: nowait postinstall skipifsilent
