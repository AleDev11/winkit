; Instalador de Winkit con Inno Setup.
;
; No usa el asistente de Inno: la ventana es propia, sin bordes y con las esquinas
; redondeadas de Windows 11, y todo lo que se ve (navegación, tarjetas, botones,
; interruptores, barra de progreso y textos) se dibuja con GDI+ en ui.iss, con los
; mismos tokens de Fluent que la app (src/styles/tokens.css). Coge el acento y el
; modo claro u oscuro de Windows.
;
; Compilar (después de `npm run tauri build -- --no-bundle`):
;   ISCC installer\winkit.iss
; Opcional: /DAppVersion=0.3.0 /DAppExe=ruta\a\winkit.exe
;
; Vista previa sin instalar nada: compila con /DPREVIEW y abre el .exe con
; /DEMO=welcome|options|cancel|installing|update|done|error y, si quieres,
; /THEME=light|dark y /ACCENT=0078D4.

#ifndef AppExe
  #define AppExe "..\src-tauri\target\release\winkit.exe"
#endif
#ifndef AppVersion
  #if FileExists(AppExe)
    #define AppVersion GetVersionNumbersString(AppExe)
  #else
    #define AppVersion "0.3.0"
  #endif
#endif
#if FileExists(AppExe)
  #define AppSizeMB Int(FileSize(AppExe) / 1048576) + 1
#else
  #define AppSizeMB 20
#endif

[Setup]
#ifdef PREVIEW
AppId={{6B0E3D1F-55A2-4C7B-9E4A-0C1F2B7D9A31}
DefaultDirName={localappdata}\WinkitPreview
Uninstallable=no
CreateUninstallRegKey=no
OutputBaseFilename=Winkit_{#AppVersion}_preview
#else
AppId={{4E8F2A6C-91B3-4D57-A0E2-7C5B3F19D864}
DefaultDirName={localappdata}\Winkit
OutputBaseFilename=Winkit_{#AppVersion}_x64-setup
#endif
AppName=Winkit
AppVersion={#AppVersion}
AppVerName=Winkit {#AppVersion}
AppPublisher=Alejandro Font
AppPublisherURL=https://github.com/AleDev11/winkit
AppSupportURL=https://github.com/AleDev11/winkit/issues
AppUpdatesURL=https://github.com/AleDev11/winkit/releases
AppCopyright=MIT
VersionInfoVersion={#AppVersion}
VersionInfoDescription=Instalador de Winkit
; Igual que el NSIS de antes: solo para el usuario actual, sin pedir administrador.
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
DisableWelcomePage=yes
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
UsePreviousAppDir=yes
ShowLanguageDialog=no
CloseApplications=no
RestartApplications=no
SetupIconFile=..\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\Winkit.exe
UninstallDisplayName=Winkit
OutputDir=Output
Compression=lzma2/ultra64
SolidCompression=yes
; El asistente de Inno queda oculto debajo de la ventana propia; el estilo
; dinámico solo afecta a los avisos del sistema y al desinstalador.
WizardStyle=modern dynamic

[Languages]
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "..\src-tauri\icons\128x128@2x.png"; DestName: "winkit-logo.png"; Flags: dontcopy
#ifdef PREVIEW
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
#else
Source: "{#AppExe}"; DestDir: "{app}"; DestName: "Winkit.exe"; Flags: ignoreversion
#endif

#ifndef PREVIEW
[Icons]
Name: "{userprograms}\Winkit"; Filename: "{app}\Winkit.exe"
Name: "{userdesktop}\Winkit"; Filename: "{app}\Winkit.exe"; Check: WantDesktopIcon
#endif

[CustomMessages]
es.Caption=Instalar Winkit
en.Caption=Install Winkit
es.NavWelcome=Bienvenida
en.NavWelcome=Welcome
es.NavOptions=Opciones
en.NavOptions=Options
es.NavInstall=Instalación
en.NavInstall=Installation
es.NavDone=Listo
en.NavDone=Done
es.WelcomeTitle=Te damos la bienvenida
en.WelcomeTitle=Welcome to Winkit
es.WelcomeIntro=Winkit pone a mano lo que más usas en Windows. Todo funciona con atajos de teclado, desde cualquier app.
en.WelcomeIntro=Winkit keeps the things you use most in Windows at hand. Everything works with keyboard shortcuts, from any app.
es.FeatPalette=Paleta
en.FeatPalette=Palette
es.FeatPaletteDesc=Corrige, traduce o reescribe lo que tengas seleccionado.
en.FeatPaletteDesc=Fix, translate or rewrite whatever you have selected.
es.FeatSearch=Buscador
en.FeatSearch=Search
es.FeatSearchDesc=Apps, cálculos, unidades y la web en un solo cuadro.
en.FeatSearchDesc=Apps, math, units and the web in a single box.
es.FeatClips=Clips
en.FeatClips=Clips
es.FeatClipsDesc=Guarda los últimos segundos de pantalla cuando quieras.
en.FeatClipsDesc=Save the last seconds of your screen whenever you want.
es.KeySpace=Espacio
en.KeySpace=Space
es.FeatColor=Selector de color
en.FeatColor=Color picker
es.FeatColorDesc=Copia el color de cualquier píxel en HEX, RGB o HSL.
en.FeatColorDesc=Copy the color of any pixel as HEX, RGB or HSL.
es.OptionsTitle=Opciones
en.OptionsTitle=Options
es.GroupLocation=Ubicación
en.GroupLocation=Location
es.DirTitle=Carpeta de instalación
en.DirTitle=Install folder
es.DirDesc=Solo para tu usuario, sin permisos de administrador.
en.DirDesc=Just for you, no administrator rights needed.
es.Browse=Examinar
en.Browse=Browse
es.BrowsePrompt=Elige dónde instalar Winkit:
en.BrowsePrompt=Choose where to install Winkit:
es.GroupShortcuts=Accesos directos
en.GroupShortcuts=Shortcuts
es.DesktopTitle=Acceso directo en el escritorio
en.DesktopTitle=Desktop shortcut
es.DesktopDesc=El del menú Inicio se crea siempre.
en.DesktopDesc=The Start menu one is always created.
es.LaunchTitle=Abrir Winkit al terminar
en.LaunchTitle=Open Winkit when done
es.LaunchDesc=Se queda en la bandeja del sistema, junto al reloj.
en.LaunchDesc=It lives in the system tray, next to the clock.
es.Space=Necesita %1 MB · Solo para tu usuario
en.Space=Needs %1 MB · Just for you
es.BtnInstall=Instalar
en.BtnInstall=Install
es.BtnOptions=Opciones
en.BtnOptions=Options
es.BtnBack=Atrás
en.BtnBack=Back
es.BtnCancel=Cancelar
en.BtnCancel=Cancel
es.BtnFinish=Terminar
en.BtnFinish=Finish
es.BtnClose=Cerrar
en.BtnClose=Close
es.InstallingTitle=Instalando Winkit
en.InstallingTitle=Installing Winkit
es.UpdatingTitle=Actualizando Winkit
en.UpdatingTitle=Updating Winkit
es.InstallingIntro=Solo tarda unos segundos.
en.InstallingIntro=This only takes a few seconds.
es.UpdatingIntro=Winkit se volverá a abrir en cuanto termine.
en.UpdatingIntro=Winkit will open again as soon as it is done.
es.Preparing=Preparando
en.Preparing=Preparing
es.Copying=Copiando archivos
en.Copying=Copying files
es.WebView=Instalando Microsoft WebView2
en.WebView=Installing Microsoft WebView2
es.TipTitle=Mientras tanto
en.TipTitle=Meanwhile
es.TipDesc=Selecciona un texto en cualquier app y pulsa este atajo.
en.TipDesc=Select some text in any app and press this shortcut.
es.DoneTitle=Todo listo
en.DoneTitle=All set
es.DoneHead=Winkit %1 está instalado
en.DoneHead=Winkit %1 is installed
es.DoneDesc=Lo tienes en el menú Inicio y en la bandeja del sistema.
en.DoneDesc=You will find it in the Start menu and in the system tray.
es.RunNowTitle=Abrir Winkit ahora
en.RunNowTitle=Open Winkit now
es.RunNowDesc=La primera vez te lleva a Ajustes para elegir tu IA.
en.RunNowDesc=The first time, it takes you to Settings to choose your AI.
es.ErrorTitle=No se ha podido instalar
en.ErrorTitle=Installation failed
es.DlgTitle=¿Salir del instalador?
en.DlgTitle=Quit the installer?
es.DlgBody=Winkit no se instalará. Puedes volver a abrir el instalador cuando quieras.
en.DlgBody=Winkit will not be installed. You can open the installer again whenever you like.
es.DlgBodyInstalling=Se deshará lo que ya se ha copiado y Winkit no quedará instalado.
en.DlgBodyInstalling=Whatever was already copied will be undone and Winkit will not be installed.
es.DlgQuit=Salir
en.DlgQuit=Quit
es.DlgStay=Seguir instalando
en.DlgStay=Keep installing

[Code]
#include "ui.iss"
