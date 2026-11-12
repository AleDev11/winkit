{ Interfaz propia del instalador de Winkit.

  Una sola superficie (TBitmapImage) que se repinta entera con GDI+ cada vez que
  cambia algo: hover, pulsación, foco, progreso o pantalla. Los clics se resuelven
  con una lista de zonas que se rellena al pintar. Inno no da eventos de ratón,
  así que el hover, la pulsación y el arrastre de la ventana salen de un
  temporizador que lee el cursor. Lo único nativo es el campo de la carpeta.

  Las medidas y los colores son los de la app: src/styles/tokens.css,
  src/styles/base.css y src/windows/settings/settings.css. }

const
  { Ventana, en píxeles lógicos (a 100 %). }
  WIN_W = 800;
  WIN_H = 540;
  NAV_W = 248;
  CONTENT_X = 272;
  CONTENT_R = 768;
  TITLE_H = 40;

  SCR_WELCOME = 0;
  SCR_OPTIONS = 1;
  SCR_INSTALLING = 2;
  SCR_DONE = 3;
  SCR_ERROR = 4;

  HIT_NONE = 0;
  HIT_MIN = 1;
  HIT_CLOSE = 2;
  HIT_NAV = 10;
  HIT_PRIMARY = 20;
  HIT_SECONDARY = 21;
  HIT_BROWSE = 22;
  HIT_DESKTOP = 23;
  HIT_LAUNCH = 24;
  HIT_RUNNOW = 25;
  HIT_DLG_STAY = 30;
  HIT_DLG_QUIT = 31;
  HIT_EDIT = 40;

  { GDI+ }
  UNIT_PIXEL = 2;
  SMOOTH_ANTIALIAS = 4;
  PIXEL_OFFSET_HALF = 4;
  TEXT_CLEARTYPE = 5;
  INTERP_HQ_BICUBIC = 7;
  ALIGN_NEAR = 0;
  ALIGN_CENTER = 1;
  ALIGN_FAR = 2;
  FMT_NOWRAP = $1000;
  FMT_NOCLIP = $4000;
  FMT_LINELIMIT = $2000;
  TRIM_ELLIPSIS = 3;
  TRIM_PATH = 5;
  FONT_REGULAR = 0;
  FONT_BOLD = 1;

  { Win32 }
  WM_NCLBUTTONDOWN = $A1;
  WM_SYSCOMMAND = $112;
  SC_MINIMIZE = $F020;
  HTCAPTION = 2;
  GWL_STYLE = -16;
  WS_SYSMENU = $80000;
  WS_MINIMIZEBOX = $20000;
  DWMWA_USE_IMMERSIVE_DARK_MODE = 20;
  DWMWA_WINDOW_CORNER_PREFERENCE = 33;
  DWMWA_BORDER_COLOR = 34;
  DWMWCP_ROUND = 2;
  VK_LBUTTON = 1;
  VK_RETURN = 13;
  VK_ESCAPE = 27;

  { Glifos de Segoe Fluent Icons (los mismos que src/components/Icon.tsx). }
  GL_HOME = #$E80F;
  GL_SETTINGS = #$E713;
  GL_DOWNLOAD = #$E896;
  GL_CHECKCIRCLE = #$EC61;
  GL_CHECK = #$E73E;
  GL_SPARKLE = #$F4A5;
  GL_SEARCH = #$E721;
  GL_RECORD = #$E7C8;
  GL_COLOR = #$E790;
  GL_FOLDER = #$E8B7;
  GL_MONITOR = #$E7F4;
  GL_PLAY = #$E768;
  GL_KEYBOARD = #$E765;
  GL_ERROR = #$E783;
  GL_CHROME_MIN = #$E921;
  GL_CHROME_CLOSE = #$E8BB;

type
  TPt = record
    X, Y: Longint;
  end;
  TRectF = record
    X, Y, W, H: Single;
  end;
  TGdipInput = record
    Version, Callback, NoThread, NoCodecs: Longword;
  end;

{ ── GDI+ ─────────────────────────────────────────────────────────── }

function GdiplusStartup(var Token: Longword; var Input: TGdipInput; Output: Longword): Integer; external 'GdiplusStartup@gdiplus.dll stdcall';
procedure GdiplusShutdown(Token: Longword); external 'GdiplusShutdown@gdiplus.dll stdcall';
function GdipCreateFromHDC(DC: Longword; var G: Longword): Integer; external 'GdipCreateFromHDC@gdiplus.dll stdcall';
function GdipDeleteGraphics(G: Longword): Integer; external 'GdipDeleteGraphics@gdiplus.dll stdcall';
function GdipSetSmoothingMode(G: Longword; Mode: Integer): Integer; external 'GdipSetSmoothingMode@gdiplus.dll stdcall';
function GdipSetPixelOffsetMode(G: Longword; Mode: Integer): Integer; external 'GdipSetPixelOffsetMode@gdiplus.dll stdcall';
function GdipSetTextRenderingHint(G: Longword; Mode: Integer): Integer; external 'GdipSetTextRenderingHint@gdiplus.dll stdcall';
function GdipSetInterpolationMode(G: Longword; Mode: Integer): Integer; external 'GdipSetInterpolationMode@gdiplus.dll stdcall';
function GdipGraphicsClear(G: Longword; Color: Cardinal): Integer; external 'GdipGraphicsClear@gdiplus.dll stdcall';
function GdipSetClipPath(G, Path: Longword; Mode: Integer): Integer; external 'GdipSetClipPath@gdiplus.dll stdcall';
function GdipResetClip(G: Longword): Integer; external 'GdipResetClip@gdiplus.dll stdcall';
function GdipCreateSolidFill(Color: Cardinal; var Brush: Longword): Integer; external 'GdipCreateSolidFill@gdiplus.dll stdcall';
function GdipDeleteBrush(Brush: Longword): Integer; external 'GdipDeleteBrush@gdiplus.dll stdcall';
function GdipCreatePen1(Color: Cardinal; Width: Single; AUnit: Integer; var Pen: Longword): Integer; external 'GdipCreatePen1@gdiplus.dll stdcall';
function GdipDeletePen(Pen: Longword): Integer; external 'GdipDeletePen@gdiplus.dll stdcall';
function GdipCreatePath(FillMode: Integer; var Path: Longword): Integer; external 'GdipCreatePath@gdiplus.dll stdcall';
function GdipDeletePath(Path: Longword): Integer; external 'GdipDeletePath@gdiplus.dll stdcall';
function GdipAddPathArc(Path: Longword; X, Y, W, H, StartAngle, Sweep: Single): Integer; external 'GdipAddPathArc@gdiplus.dll stdcall';
function GdipAddPathRectangle(Path: Longword; X, Y, W, H: Single): Integer; external 'GdipAddPathRectangle@gdiplus.dll stdcall';
function GdipClosePathFigure(Path: Longword): Integer; external 'GdipClosePathFigure@gdiplus.dll stdcall';
function GdipFillPath(G, Brush, Path: Longword): Integer; external 'GdipFillPath@gdiplus.dll stdcall';
function GdipDrawPath(G, Pen, Path: Longword): Integer; external 'GdipDrawPath@gdiplus.dll stdcall';
function GdipFillEllipse(G, Brush: Longword; X, Y, W, H: Single): Integer; external 'GdipFillEllipse@gdiplus.dll stdcall';
function GdipFillRectangle(G, Brush: Longword; X, Y, W, H: Single): Integer; external 'GdipFillRectangle@gdiplus.dll stdcall';
function GdipCreateFontFamilyFromName(Name: String; Collection: Longword; var Family: Longword): Integer; external 'GdipCreateFontFamilyFromName@gdiplus.dll stdcall';
function GdipCreateFont(Family: Longword; Size: Single; Style: Integer; AUnit: Integer; var Font: Longword): Integer; external 'GdipCreateFont@gdiplus.dll stdcall';
function GdipDeleteFont(Font: Longword): Integer; external 'GdipDeleteFont@gdiplus.dll stdcall';
function GdipStringFormatGetGenericTypographic(var Format: Longword): Integer; external 'GdipStringFormatGetGenericTypographic@gdiplus.dll stdcall';
function GdipCloneStringFormat(Format: Longword; var Clone: Longword): Integer; external 'GdipCloneStringFormat@gdiplus.dll stdcall';
function GdipSetStringFormatFlags(Format: Longword; Flags: Integer): Integer; external 'GdipSetStringFormatFlags@gdiplus.dll stdcall';
function GdipSetStringFormatAlign(Format: Longword; Align: Integer): Integer; external 'GdipSetStringFormatAlign@gdiplus.dll stdcall';
function GdipSetStringFormatLineAlign(Format: Longword; Align: Integer): Integer; external 'GdipSetStringFormatLineAlign@gdiplus.dll stdcall';
function GdipSetStringFormatTrimming(Format: Longword; Trimming: Integer): Integer; external 'GdipSetStringFormatTrimming@gdiplus.dll stdcall';
function GdipDrawString(G: Longword; Str: String; Len: Integer; Font: Longword; var Layout: TRectF; Format, Brush: Longword): Integer; external 'GdipDrawString@gdiplus.dll stdcall';
function GdipMeasureString(G: Longword; Str: String; Len: Integer; Font: Longword; var Layout: TRectF; Format: Longword; var Box: TRectF; Fitted, Lines: Longword): Integer; external 'GdipMeasureString@gdiplus.dll stdcall';
function GdipLoadImageFromFile(FileName: String; var Image: Longword): Integer; external 'GdipLoadImageFromFile@gdiplus.dll stdcall';
function GdipDrawImageRect(G, Image: Longword; X, Y, W, H: Single): Integer; external 'GdipDrawImageRect@gdiplus.dll stdcall';

{ ── Win32 ────────────────────────────────────────────────────────── }

function DwmSetWindowAttribute(Wnd: Longword; Attr: Integer; var Value: Integer; Size: Integer): Integer; external 'DwmSetWindowAttribute@dwmapi.dll stdcall delayload';
function CreateRoundRectRgn(X1, Y1, X2, Y2, W, H: Integer): Longword; external 'CreateRoundRectRgn@gdi32.dll stdcall';
function SetWindowRgn(Wnd, Rgn: Longword; Redraw: Integer): Integer; external 'SetWindowRgn@user32.dll stdcall';
function GetWindowLongW(Wnd: Longword; Index: Integer): Longint; external 'GetWindowLongW@user32.dll stdcall';
function SetWindowLongW(Wnd: Longword; Index: Integer; Value: Longint): Longint; external 'SetWindowLongW@user32.dll stdcall';
function GetCursorPos(var P: TPt): Integer; external 'GetCursorPos@user32.dll stdcall';
function ScreenToClient(Wnd: Longword; var P: TPt): Integer; external 'ScreenToClient@user32.dll stdcall';
function GetAsyncKeyState(Key: Integer): Integer; external 'GetAsyncKeyState@user32.dll stdcall';
function ReleaseCapture: Integer; external 'ReleaseCapture@user32.dll stdcall';
function GetForegroundWindow: Longword; external 'GetForegroundWindow@user32.dll stdcall';
function SetTimer(Wnd, Id, Elapse, Proc: Longword): Longword; external 'SetTimer@user32.dll stdcall';
function KillTimer(Wnd, Id: Longword): Integer; external 'KillTimer@user32.dll stdcall';

var
  S: Single;
  Host: TPanel;
  Surface: TBitmapImage;
  DirBox: TNewEdit;
  MainPage: TWizardPage;

  GdipToken, G: Longword;
  FTitle, FSub, FBody, FStrong, FCaption, FIcon, FIconSm, FIconLg: Longword;
  FmtLeft, FmtCenter, FmtRight, FmtWrap, FmtPath: Longword;
  LogoImg: Longword;

  Dark: Boolean;
  cBg, cCard, cCardHover, cCardStroke, cT1, cT2, cT3, cTDis: Cardinal;
  cSubHover, cSubPressed: Cardinal;
  cCtrl, cCtrlHover, cCtrlPressed, cInputActive: Cardinal;
  cStrokeCtrl, cStrokeCtrl2, cStrokeStrong, cCtrlAlt, cCtrlAltHover: Cardinal;
  cAccentFill, cAccentText, cOnAccent, cOnAccent2, cStrokeAccent2: Cardinal;
  cSurfaceStroke, cDialog, cCritical: Cardinal;

  CurScreen, Hover, PressId: Integer;
  Down, DialogOpen, ForceCancel, EditFocused: Boolean;
  OptDesktop, OptLaunch, RunNow: Boolean;
  UpdateMode, Relaunch, PendingNext, PendingFinish: Boolean;
  Progress: Single;
  StatusText, FileText, ErrorText: String;
  Demo: String;
  Ticking, CornersDone: Boolean;
  TimerId: Longword;

  HitId, HitX, HitY, HitW, HitH: array of Integer;
  HitCount: Integer;

{ ── Colores (ARGB) ───────────────────────────────────────────────── }

function ARGB(A, R, Gr, B: Integer): Cardinal;
begin
  Result := (Cardinal(A) shl 24) or (Cardinal(R) shl 16) or (Cardinal(Gr) shl 8) or Cardinal(B);
end;

function Ch(C: Cardinal; Shift: Integer): Integer;
begin
  Result := (C shr Shift) and $FF;
end;

function Hex(RGB: Cardinal): Cardinal;
begin
  Result := $FF000000 or RGB;
end;

{ Mismo color con otra opacidad, como rgba(). }
function Alpha(C: Cardinal; A: Single): Cardinal;
begin
  Result := (Cardinal(Round(A * 255)) shl 24) or (C and $FFFFFF);
end;

{ color-mix(): T de C1 y el resto de C2. }
function Mix(C1, C2: Cardinal; T: Single): Cardinal;
begin
  Result := ARGB(255,
    Round(Ch(C1, 16) * T + Ch(C2, 16) * (1 - T)),
    Round(Ch(C1, 8) * T + Ch(C2, 8) * (1 - T)),
    Round(Ch(C1, 0) * T + Ch(C2, 0) * (1 - T)));
end;

{ Un color translúcido ya compuesto sobre un fondo opaco. }
function Over(Fg, Bg: Cardinal): Cardinal;
begin
  Result := Mix(Fg, Bg, Ch(Fg, 24) / 255.0);
end;

function ToTColor(C: Cardinal): TColor;
begin
  Result := (Ch(C, 0) shl 16) or (Ch(C, 8) shl 8) or Ch(C, 16);
end;

{ El acento base de Windows (SystemAccentColor), del mismo sitio que lo lee la app. }
function SystemAccent: Cardinal;
var
  Palette: AnsiString;
  V: Cardinal;
begin
  Result := Hex($0078D4);
  if RegQueryBinaryValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Explorer\Accent', 'AccentPalette', Palette) and (Length(Palette) >= 16) then
    Result := ARGB(255, Ord(Palette[13]), Ord(Palette[14]), Ord(Palette[15]))
  else if RegQueryDWordValue(HKCU, 'Software\Microsoft\Windows\DWM', 'AccentColor', V) then
    Result := ARGB(255, V and $FF, (V shr 8) and $FF, (V shr 16) and $FF);
end;

function SystemIsDark: Boolean;
var
  V: Cardinal;
begin
  Result := RegQueryDWordValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Themes\Personalize', 'AppsUseLightTheme', V) and (V = 0);
end;

procedure LoadTheme;
var
  Theme: String;
  Accent, White, Black: Cardinal;
begin
  Theme := Lowercase(ExpandConstant('{param:THEME|}'));
  if Theme = 'dark' then Dark := True
  else if Theme = 'light' then Dark := False
  else Dark := SystemIsDark;

  Accent := SystemAccent;
  { Para capturas con un acento fijo: /ACCENT=0078D4 }
  if ExpandConstant('{param:ACCENT|}') <> '' then
    Accent := Hex(StrToIntDef('$' + ExpandConstant('{param:ACCENT|}'), $0078D4));
  White := Hex($FFFFFF);
  Black := Hex($000000);

  if Dark then
  begin
    cBg := Hex($202020);
    cCard := Over(Alpha(White, 0.0512), cBg);
    cCardHover := Over(Alpha(White, 0.0698), cBg);
    cCardStroke := Over(Alpha(Black, 0.1), cBg);
    cT1 := White;
    cT2 := Over(Alpha(White, 0.786), cBg);
    cT3 := Over(Alpha(White, 0.544), cBg);
    cTDis := Over(Alpha(White, 0.3628), cBg);
    cSubHover := Alpha(White, 0.0605);
    cSubPressed := Alpha(White, 0.0419);
    cCtrl := Alpha(White, 0.0605);
    cCtrlHover := Alpha(White, 0.0837);
    cCtrlPressed := Alpha(White, 0.0326);
    cInputActive := Alpha(Hex($1E1E1E), 0.7);
    cStrokeCtrl := Alpha(White, 0.0698);
    cStrokeCtrl2 := Alpha(White, 0.093);
    cStrokeStrong := Alpha(White, 0.5442);
    cCtrlAlt := Alpha(Black, 0.1);
    cCtrlAltHover := Alpha(White, 0.0419);
    cAccentFill := Mix(Accent, White, 0.52);
    cAccentText := Mix(Accent, White, 0.45);
    cOnAccent := Black;
    cOnAccent2 := Alpha(Black, 0.5);
    cStrokeAccent2 := Alpha(Black, 0.14);
    cSurfaceStroke := Alpha(Hex($757575), 0.4);
    cDialog := Hex($2B2B2B);
    cCritical := Hex($C42B1C);
  end
  else
  begin
    cBg := Hex($F3F3F3);
    cCard := Over(Alpha(White, 0.7), cBg);
    cCardHover := Over(Alpha(Hex($F9F9F9), 0.5), cBg);
    cCardStroke := Over(Alpha(Black, 0.0578), cBg);
    cT1 := Over(Alpha(Black, 0.896), cBg);
    cT2 := Over(Alpha(Black, 0.606), cBg);
    cT3 := Over(Alpha(Black, 0.446), cBg);
    cTDis := Over(Alpha(Black, 0.361), cBg);
    cSubHover := Alpha(Black, 0.0373);
    cSubPressed := Alpha(Black, 0.0241);
    cCtrl := Alpha(White, 0.7);
    cCtrlHover := Alpha(Hex($F9F9F9), 0.5);
    cCtrlPressed := Alpha(Hex($F9F9F9), 0.3);
    cInputActive := White;
    cStrokeCtrl := Alpha(Black, 0.0578);
    cStrokeCtrl2 := Alpha(Black, 0.1622);
    cStrokeStrong := Alpha(Black, 0.446);
    cCtrlAlt := Alpha(Black, 0.0241);
    cCtrlAltHover := Alpha(Black, 0.0578);
    cAccentFill := Mix(Accent, Black, 0.8);
    cAccentText := Mix(Accent, Black, 0.72);
    cOnAccent := White;
    cOnAccent2 := Alpha(White, 0.7);
    cStrokeAccent2 := Alpha(Black, 0.4);
    cSurfaceStroke := Alpha(Hex($757575), 0.4);
    cDialog := White;
    cCritical := Hex($C42B1C);
  end;
end;

{ ── Fuentes ──────────────────────────────────────────────────────── }

{ Primera familia que exista de una lista separada por «|». }
function FindFamily(Names: String): Longword;
var
  Name: String;
  P: Integer;
begin
  Result := 0;
  while (Result = 0) and (Names <> '') do
  begin
    P := Pos('|', Names);
    if P > 0 then
    begin
      Name := Copy(Names, 1, P - 1);
      Delete(Names, 1, P);
    end
    else
    begin
      Name := Names;
      Names := '';
    end;
    if GdipCreateFontFamilyFromName(Name, 0, Result) <> 0 then
      Result := 0
    else
      Log('Fuente: ' + Name);
  end;
end;

function MakeFont(Names: String; Px: Single; Style: Integer): Longword;
var
  Family: Longword;
begin
  Result := 0;
  Family := FindFamily(Names);
  if Family = 0 then Family := FindFamily('Segoe UI|Tahoma');
  GdipCreateFont(Family, Px * S, Style, UNIT_PIXEL, Result);
end;

function MakeFormat(Align, Flags, Trimming: Integer): Longword;
var
  Base: Longword;
begin
  Result := 0;
  GdipStringFormatGetGenericTypographic(Base);
  GdipCloneStringFormat(Base, Result);
  GdipSetStringFormatFlags(Result, Flags);
  GdipSetStringFormatAlign(Result, Align);
  GdipSetStringFormatLineAlign(Result, ALIGN_CENTER);
  GdipSetStringFormatTrimming(Result, Trimming);
end;

procedure LoadFonts;
var
  Semi: Integer;
  SemiDisplay, SemiText: String;
begin
  { Segoe UI Variable expone sus pesos como familias. En Windows 10 no existe:
    se cae a Segoe UI Semibold, y si tampoco, a negrita. }
  SemiDisplay := 'Segoe UI Variable Display Semib|Segoe UI Variable Display Semibold|Segoe UI Semibold';
  SemiText := 'Segoe UI Variable Text Semibold|Segoe UI Variable Text Semib|Segoe UI Semibold';
  if FindFamily(SemiDisplay) = 0 then Semi := FONT_BOLD else Semi := FONT_REGULAR;

  FTitle := MakeFont(SemiDisplay, 28, Semi);
  FSub := MakeFont(SemiDisplay, 20, Semi);
  FStrong := MakeFont(SemiText, 14, Semi);
  FBody := MakeFont('Segoe UI Variable Text|Segoe UI', 14, FONT_REGULAR);
  FCaption := MakeFont('Segoe UI Variable Small|Segoe UI Variable Text|Segoe UI', 12, FONT_REGULAR);
  FIcon := MakeFont('Segoe Fluent Icons|Segoe MDL2 Assets', 16, FONT_REGULAR);
  FIconSm := MakeFont('Segoe Fluent Icons|Segoe MDL2 Assets', 10, FONT_REGULAR);
  FIconLg := MakeFont('Segoe Fluent Icons|Segoe MDL2 Assets', 20, FONT_REGULAR);

  FmtLeft := MakeFormat(ALIGN_NEAR, FMT_NOWRAP or FMT_NOCLIP, TRIM_ELLIPSIS);
  FmtCenter := MakeFormat(ALIGN_CENTER, FMT_NOWRAP or FMT_NOCLIP, TRIM_ELLIPSIS);
  FmtRight := MakeFormat(ALIGN_FAR, FMT_NOWRAP or FMT_NOCLIP, TRIM_ELLIPSIS);
  FmtPath := MakeFormat(ALIGN_NEAR, FMT_NOWRAP, TRIM_PATH);
  FmtWrap := MakeFormat(ALIGN_NEAR, FMT_LINELIMIT, TRIM_ELLIPSIS);
  GdipSetStringFormatLineAlign(FmtWrap, ALIGN_NEAR);
end;

{ ── Primitivas (en píxeles lógicos) ─────────────────────────────── }

function Px(V: Single): Single;
begin
  Result := Round(V * S);
end;

procedure AddRound(Path: Longword; X, Y, W, H, R: Single);
var
  D: Single;
begin
  if R <= 0 then
  begin
    GdipAddPathRectangle(Path, X, Y, W, H);
    Exit;
  end;
  D := R * 2;
  GdipAddPathArc(Path, X, Y, D, D, 180, 90);
  GdipAddPathArc(Path, X + W - D, Y, D, D, 270, 90);
  GdipAddPathArc(Path, X + W - D, Y + H - D, D, D, 0, 90);
  GdipAddPathArc(Path, X, Y + H - D, D, D, 90, 90);
  GdipClosePathFigure(Path);
end;

{ Rectángulo redondeado en píxeles físicos. }
procedure FillRoundPx(X, Y, W, H, R: Single; C: Cardinal);
var
  Path, Brush: Longword;
begin
  if (W <= 0) or (H <= 0) then Exit;
  GdipCreatePath(0, Path);
  AddRound(Path, X, Y, W, H, R);
  GdipCreateSolidFill(C, Brush);
  GdipFillPath(G, Brush, Path);
  GdipDeleteBrush(Brush);
  GdipDeletePath(Path);
end;

procedure FillRound(X, Y, W, H, R: Single; C: Cardinal);
begin
  FillRoundPx(Px(X), Px(Y), Px(X + W) - Px(X), Px(Y + H) - Px(Y), R * S, C);
end;

{ Trazo de 1 px físico por dentro del borde, como un border de CSS. }
procedure StrokeRound(X, Y, W, H, R: Single; C: Cardinal);
var
  Path, Pen: Longword;
  X1, Y1: Single;
begin
  X1 := Px(X);
  Y1 := Px(Y);
  GdipCreatePath(0, Path);
  AddRound(Path, X1 + 0.5, Y1 + 0.5, Px(X + W) - X1 - 1, Px(Y + H) - Y1 - 1, R * S - 0.5);
  GdipCreatePen1(C, 1, UNIT_PIXEL, Pen);
  GdipDrawPath(G, Pen, Path);
  GdipDeletePen(Pen);
  GdipDeletePath(Path);
end;

{ Borde de elevación de Fluent: un trazo arriba y otro distinto abajo. Colores opacos. }
procedure Elevated(X, Y, W, H, R: Single; Fill, Top, Bottom: Cardinal);
var
  X1, Y1, W1, H1, Rp: Single;
begin
  X1 := Px(X);
  Y1 := Px(Y);
  W1 := Px(X + W) - X1;
  H1 := Px(Y + H) - Y1;
  Rp := R * S;
  FillRoundPx(X1, Y1, W1, H1, Rp, Bottom);
  FillRoundPx(X1, Y1, W1, H1 - 1, Rp, Top);
  FillRoundPx(X1 + 1, Y1 + 1, W1 - 2, H1 - 2, Rp - 1, Fill);
end;

procedure FillCircle(CX, CY, D: Single; C: Cardinal);
var
  Brush: Longword;
begin
  GdipCreateSolidFill(C, Brush);
  GdipFillEllipse(G, Brush, CX * S - D * S / 2, CY * S - D * S / 2, D * S, D * S);
  GdipDeleteBrush(Brush);
end;

procedure FillRect(X, Y, W, H: Single; C: Cardinal);
var
  Brush: Longword;
begin
  GdipCreateSolidFill(C, Brush);
  GdipFillRectangle(G, Brush, Px(X), Px(Y), Px(X + W) - Px(X), Px(Y + H) - Px(Y));
  GdipDeleteBrush(Brush);
end;

procedure Txt(T: String; Font: Longword; C: Cardinal; X, Y, W, H: Single; Fmt: Longword);
var
  R: TRectF;
  Brush: Longword;
begin
  if T = '' then Exit;
  R.X := X * S;
  R.Y := Y * S;
  R.W := W * S;
  R.H := H * S;
  GdipCreateSolidFill(C, Brush);
  GdipDrawString(G, T, Length(T), Font, R, Fmt, Brush);
  GdipDeleteBrush(Brush);
end;

function TextWidth(T: String; Font: Longword): Single;
var
  R, Box: TRectF;
begin
  R.X := 0;
  R.Y := 0;
  R.W := 10000;
  R.H := 1000;
  Box.W := 0;
  GdipMeasureString(G, T, Length(T), Font, R, FmtLeft, Box, 0, 0);
  Result := Box.W / S;
end;

procedure Glyph(Code: String; Font: Longword; C: Cardinal; CX, CY, Box: Single);
begin
  Txt(Code, Font, C, CX - Box / 2, CY - Box / 2, Box, Box, FmtCenter);
end;

{ ── Zonas de clic ────────────────────────────────────────────────── }

procedure AddHit(Id: Integer; X, Y, W, H: Single);
begin
  if HitCount >= GetArrayLength(HitId) then
  begin
    SetArrayLength(HitId, HitCount + 16);
    SetArrayLength(HitX, HitCount + 16);
    SetArrayLength(HitY, HitCount + 16);
    SetArrayLength(HitW, HitCount + 16);
    SetArrayLength(HitH, HitCount + 16);
  end;
  HitId[HitCount] := Id;
  HitX[HitCount] := Round(X);
  HitY[HitCount] := Round(Y);
  HitW[HitCount] := Round(W);
  HitH[HitCount] := Round(H);
  HitCount := HitCount + 1;
end;

{ La última zona añadida gana: así un diálogo tapa lo que hay debajo. }
function HitTest(X, Y: Single): Integer;
var
  I: Integer;
begin
  Result := HIT_NONE;
  for I := HitCount - 1 downto 0 do
    if (X >= HitX[I]) and (X < HitX[I] + HitW[I]) and (Y >= HitY[I]) and (Y < HitY[I] + HitH[I]) then
    begin
      Result := HitId[I];
      Exit;
    end;
end;

function IsHover(Id: Integer): Boolean;
begin
  Result := (Hover = Id) and (Id <> HIT_NONE);
end;

function IsPressed(Id: Integer): Boolean;
begin
  Result := IsHover(Id) and Down and (PressId = Id);
end;

{ ── Componentes ──────────────────────────────────────────────────── }

{ .btn y .btn.accent de base.css. }
procedure Button(Id: Integer; X, Y, W: Single; Caption: String; Accent: Boolean; Under: Cardinal);
var
  Fill, Top, Bottom, Fg: Cardinal;
begin
  if Accent then
  begin
    Fill := cAccentFill;
    if IsPressed(Id) then Fill := Over(Alpha(cAccentFill, 0.8), Under)
    else if IsHover(Id) then Fill := Over(Alpha(cAccentFill, 0.9), Under);
    Top := Over(Alpha(Hex($FFFFFF), 0.08), Fill);
    Bottom := Over(cStrokeAccent2, Fill);
    Fg := cOnAccent;
    if IsPressed(Id) then
    begin
      Top := Fill;
      Bottom := Fill;
      Fg := Over(cOnAccent2, Fill);
    end;
  end
  else
  begin
    Fill := Over(cCtrl, Under);
    if IsPressed(Id) then Fill := Over(cCtrlPressed, Under)
    else if IsHover(Id) then Fill := Over(cCtrlHover, Under);
    Top := Over(cStrokeCtrl, Under);
    Bottom := Over(cStrokeCtrl2, Under);
    Fg := cT1;
    if IsPressed(Id) then
    begin
      Bottom := Top;
      Fg := cT2;
    end;
  end;
  Elevated(X, Y, W, 32, 4, Fill, Top, Bottom);
  Txt(Caption, FBody, Fg, X + 8, Y, W - 16, 32, FmtCenter);
  AddHit(Id, X, Y, W, 32);
end;

function ButtonWidth(Caption: String): Single;
begin
  Result := TextWidth(Caption, FBody) + 24;
  if Result < 120 then Result := 120;
end;

{ .switch: 40 × 20, la bolita crece al pasar por encima. }
procedure Switch(X, Y: Single; On, Hot: Boolean; Under: Cardinal);
var
  Knob, KX: Single;
begin
  if Hot then Knob := 14 else Knob := 12;
  if On then
  begin
    if Hot then FillRound(X, Y, 40, 20, 10, Over(Alpha(cAccentFill, 0.9), Under))
    else FillRound(X, Y, 40, 20, 10, cAccentFill);
    if Hot then KX := X + 21 else KX := X + 22;
    FillCircle(KX + Knob / 2, Y + 10, Knob, cOnAccent);
  end
  else
  begin
    if Hot then FillRound(X, Y, 40, 20, 10, Over(cCtrlAltHover, Under))
    else FillRound(X, Y, 40, 20, 10, Over(cCtrlAlt, Under));
    StrokeRound(X, Y, 40, 20, 10, Over(cStrokeStrong, Under));
    if Hot then KX := X + 3 else KX := X + 4;
    FillCircle(KX + Knob / 2, Y + 10, Knob, cT2);
  end;
end;

{ .card }
procedure Card(X, Y, W, H: Single; Hot: Boolean);
begin
  if Hot then FillRound(X, Y, W, H, 4, cCardHover)
  else FillRound(X, Y, W, H, 4, cCard);
  StrokeRound(X, Y, W, H, 4, cCardStroke);
end;

{ .card-head: icono, título y descripción. }
procedure CardHead(X, Y, W: Single; Code, Title, Desc: String; TextW: Single);
begin
  Glyph(Code, FIcon, cT1, X + 26, Y + 34, 20);
  Txt(Title, FBody, cT1, X + 52, Y + 14, TextW, 20, FmtLeft);
  Txt(Desc, FCaption, cT2, X + 52, Y + 34, TextW, 16, FmtLeft);
end;

{ Tarjeta entera clicable con interruptor a la derecha (label.card-head). }
procedure SwitchCard(Id: Integer; X, Y, W: Single; Code, Title, Desc: String; On: Boolean);
var
  Hot: Boolean;
  Under: Cardinal;
begin
  Hot := IsHover(Id);
  Card(X, Y, W, 68, Hot);
  if Hot then Under := cCardHover else Under := cCard;
  CardHead(X, Y, W, Code, Title, Desc, W - 52 - 16 - 56);
  Switch(X + W - 16 - 40, Y + 24, On, Hot, Under);
  AddHit(Id, X, Y, W, 68);
end;

{ kbd: una tecla. Devuelve el ancho. }
function KeyWidth(Key: String): Single; forward;

function Keycap(X, Y: Single; Key: String): Single;
begin
  Result := KeyWidth(Key);
  Elevated(X, Y, Result, 20, 4, Over(cCtrl, cCard), Over(cStrokeCtrl, cCard), Over(cStrokeCtrl2, cCard));
  Txt(Key, FCaption, cT1, X, Y, Result, 20, FmtCenter);
end;

{ «Ctrl+Alt+X» → ['Ctrl', 'Alt', 'X']. }
function SplitKeys(Keys: String; var Parts: array of String): Integer;
var
  P: Integer;
begin
  Result := 0;
  while Keys <> '' do
  begin
    SetArrayLength(Parts, Result + 1);
    P := Pos('+', Keys);
    if P > 0 then
    begin
      Parts[Result] := Copy(Keys, 1, P - 1);
      Delete(Keys, 1, P);
    end
    else
    begin
      Parts[Result] := Keys;
      Keys := '';
    end;
    Result := Result + 1;
  end;
end;

function KeyWidth(Key: String): Single;
begin
  Result := TextWidth(Key, FCaption) + 12;
  if Result < 20 then Result := 20;
end;

function ShortcutWidth(Keys: String): Single;
var
  Parts: array of String;
  Count, I: Integer;
begin
  Count := SplitKeys(Keys, Parts);
  Result := 4 * (Count - 1);
  for I := 0 to Count - 1 do
    Result := Result + KeyWidth(Parts[I]);
end;

{ Atajo alineado a la derecha, una tecla por parte. }
procedure Shortcut(RightX, Y: Single; Keys: String);
var
  Parts: array of String;
  Count, I: Integer;
  X: Single;
begin
  X := RightX - ShortcutWidth(Keys);
  Count := SplitKeys(Keys, Parts);
  for I := 0 to Count - 1 do
    X := X + Keycap(X, Y, Parts[I]) + 4;
end;

{ ProgressBar de WinUI: pista de 1 px y barra de 3 px. }
procedure ProgressBar(X, Y, W, Value: Single);
begin
  FillRect(X, Y + 1, W, 1, Over(cStrokeStrong, cCard));
  if Value > 0 then
  begin
    if Value > 1 then Value := 1;
    FillRound(X, Y, W * Value, 3, 1.5, cAccentFill);
  end;
end;

{ ── Partes de la ventana ─────────────────────────────────────────── }

procedure DrawTitleBar;
var
  CloseFill, CloseFg: Cardinal;
begin
  if LogoImg <> 0 then
    GdipDrawImageRect(G, LogoImg, Px(16), Px(12), Px(16), Px(16));
  Txt(CustomMessage('Caption'), FCaption, cT2, 44, 12, 300, 16, FmtLeft);

  { Botones de la barra de título de Windows 11: 46 × 32. }
  if IsPressed(HIT_MIN) then FillRect(WIN_W - 92, 0, 46, 32, Over(cSubPressed, cBg))
  else if IsHover(HIT_MIN) then FillRect(WIN_W - 92, 0, 46, 32, Over(cSubHover, cBg));
  Glyph(GL_CHROME_MIN, FIconSm, cT1, WIN_W - 69, 16, 16);
  AddHit(HIT_MIN, WIN_W - 92, 0, 46, 32);

  CloseFg := cT1;
  if IsHover(HIT_CLOSE) then
  begin
    if IsPressed(HIT_CLOSE) then CloseFill := Over(Alpha(cCritical, 0.9), cBg) else CloseFill := cCritical;
    FillRect(WIN_W - 46, 0, 46, 32, CloseFill);
    CloseFg := Hex($FFFFFF);
  end;
  Glyph(GL_CHROME_CLOSE, FIconSm, CloseFg, WIN_W - 23, 16, 16);
  AddHit(HIT_CLOSE, WIN_W - 46, 0, 46, 32);
end;

procedure NavItem(Index: Integer; Code, Caption: String; Enabled, Selected, Done: Boolean);
var
  Id: Integer;
  Y: Single;
  Fg: Cardinal;
begin
  Id := HIT_NAV + Index;
  Y := 120 + Index * 40;
  if Selected or (Enabled and IsHover(Id)) then
  begin
    if Enabled and IsPressed(Id) then FillRound(8, Y, NAV_W - 16, 36, 4, Over(cSubPressed, cBg))
    else FillRound(8, Y, NAV_W - 16, 36, 4, Over(cSubHover, cBg));
  end;
  if Selected then
    FillRound(8, Y + 10, 3, 16, 1.5, cAccentFill);
  if Enabled or Selected then Fg := cT1 else Fg := cTDis;
  if Enabled and IsPressed(Id) then Fg := cT2;
  Glyph(Code, FIcon, Fg, 8 + 16 + 8, Y + 18, 20);
  Txt(Caption, FBody, Fg, 8 + 48, Y, NAV_W - 16 - 48 - 32, 36, FmtLeft);
  if Done then
    Glyph(GL_CHECK, FIconSm, cAccentText, NAV_W - 8 - 22, Y + 18, 16);
  if Enabled then
    AddHit(Id, 8, Y, NAV_W - 16, 36);
end;

procedure DrawNav;
var
  Before: Boolean;
begin
  { .nav-app }
  if LogoImg <> 0 then
    GdipDrawImageRect(G, LogoImg, Px(20), Px(56), Px(40), Px(40));
  Txt('Winkit', FStrong, cT1, 74, 57, 160, 20, FmtLeft);
  Txt('{#AppVersion}', FCaption, cT2, 74, 77, 160, 16, FmtLeft);

  Before := (CurScreen = SCR_WELCOME) or (CurScreen = SCR_OPTIONS);
  NavItem(0, GL_HOME, CustomMessage('NavWelcome'), Before and not DialogOpen, CurScreen = SCR_WELCOME, not Before);
  NavItem(1, GL_SETTINGS, CustomMessage('NavOptions'), Before and not DialogOpen, CurScreen = SCR_OPTIONS, not Before);
  NavItem(2, GL_DOWNLOAD, CustomMessage('NavInstall'), False, (CurScreen = SCR_INSTALLING) or (CurScreen = SCR_ERROR), CurScreen = SCR_DONE);
  NavItem(3, GL_CHECKCIRCLE, CustomMessage('NavDone'), False, CurScreen = SCR_DONE, False);
end;

procedure PageHead(Title, Intro: String);
begin
  Txt(Title, FTitle, cT1, CONTENT_X, 48, CONTENT_R - CONTENT_X, 36, FmtLeft);
  if Intro <> '' then
    Txt(Intro, FBody, cT2, CONTENT_X, 92, CONTENT_R - CONTENT_X - 40, 40, FmtWrap);
end;

procedure FeatureCard(Y: Single; Code, Title, Desc, Keys: String);
var
  W, TW: Single;
begin
  W := CONTENT_R - CONTENT_X;
  TW := W - 52 - 16 - ShortcutWidth(Keys) - 24;
  Card(CONTENT_X, Y, W, 64, False);
  Glyph(Code, FIcon, cT1, CONTENT_X + 26, Y + 32, 20);
  Txt(Title, FBody, cT1, CONTENT_X + 52, Y + 12, TW, 20, FmtLeft);
  Txt(Desc, FCaption, cT2, CONTENT_X + 52, Y + 32, TW, 16, FmtLeft);
  Shortcut(CONTENT_R - 16, Y + 22, Keys);
end;

{ Pie con la nota de la izquierda y hasta dos botones a la derecha. }
procedure Footer(Note, Secondary, Primary: String);
var
  X, WP, WS: Single;
  Y: Single;
begin
  Y := WIN_H - 24 - 32;
  X := CONTENT_R;
  if Primary <> '' then
  begin
    WP := ButtonWidth(Primary);
    X := X - WP;
    Button(HIT_PRIMARY, X, Y, WP, Primary, True, cBg);
    X := X - 8;
  end;
  if Secondary <> '' then
  begin
    WS := ButtonWidth(Secondary);
    X := X - WS;
    Button(HIT_SECONDARY, X, Y, WS, Secondary, Primary = '', cBg);
  end;
  if Note <> '' then
    Txt(Note, FCaption, cT2, CONTENT_X, Y, X - CONTENT_X - 16, 32, FmtLeft);
end;

procedure DrawWelcome;
begin
  PageHead(CustomMessage('WelcomeTitle'), CustomMessage('WelcomeIntro'));
  FeatureCard(148, GL_SPARKLE, CustomMessage('FeatPalette'), CustomMessage('FeatPaletteDesc'), 'Ctrl+Alt+X');
  FeatureCard(216, GL_SEARCH, CustomMessage('FeatSearch'), CustomMessage('FeatSearchDesc'), 'Alt+' + CustomMessage('KeySpace'));
  FeatureCard(284, GL_RECORD, CustomMessage('FeatClips'), CustomMessage('FeatClipsDesc'), 'Ctrl+Alt+G');
  FeatureCard(352, GL_COLOR, CustomMessage('FeatColor'), CustomMessage('FeatColorDesc'), 'Ctrl+Alt+P');
  Footer(FmtMessage(CustomMessage('Space'), ['{#AppSizeMB}']), CustomMessage('BtnOptions'), CustomMessage('BtnInstall'));
end;

{ Caja del campo de la carpeta (.input). El texto lo pone un TNewEdit encima. }
procedure DrawDirInput(X, Y, W: Single);
var
  Fill: Cardinal;
  Path: Longword;
  X1, Y1, W1, H1: Single;
begin
  if EditFocused then Fill := Over(cInputActive, cCard)
  else if IsHover(HIT_EDIT) then Fill := Over(cCtrlHover, cCard)
  else Fill := Over(cCtrl, cCard);
  Elevated(X, Y, W, 32, 4, Fill, Over(cStrokeCtrl, cCard), Over(cStrokeStrong, cCard));
  if EditFocused then
  begin
    { La línea de acento de 2 px, recortada por las esquinas redondeadas. }
    X1 := Px(X);
    Y1 := Px(Y);
    W1 := Px(X + W) - X1;
    H1 := Px(Y + 32) - Y1;
    GdipCreatePath(0, Path);
    AddRound(Path, X1, Y1, W1, H1, 4 * S);
    GdipSetClipPath(G, Path, 0);
    FillRect(X, Y + 30, W, 2, cAccentFill);
    GdipResetClip(G);
    GdipDeletePath(Path);
  end;
  AddHit(HIT_EDIT, X, Y, W, 32);

  if DirBox.Color <> ToTColor(Fill) then DirBox.Color := ToTColor(Fill);
  DirBox.Font.Color := ToTColor(cT1);
  DirBox.SetBounds(Round((X + 11) * S), Round((Y + 6) * S), Round((W - 22) * S), Round(20 * S));
end;

procedure DrawOptions;
var
  W, BW: Single;
begin
  W := CONTENT_R - CONTENT_X;
  PageHead(CustomMessage('OptionsTitle'), '');

  Txt(CustomMessage('GroupLocation'), FStrong, cT1, CONTENT_X + 2, 100, W, 20, FmtLeft);
  Card(CONTENT_X, 128, W, 116, False);
  CardHead(CONTENT_X, 128, W, GL_FOLDER, CustomMessage('DirTitle'), CustomMessage('DirDesc'), W - 52 - 16);
  BW := ButtonWidth(CustomMessage('Browse'));
  BW := BW - 20;
  DrawDirInput(CONTENT_X + 52, 128 + 68, W - 52 - 16 - 8 - BW);
  Button(HIT_BROWSE, CONTENT_R - 16 - BW, 128 + 68, BW, CustomMessage('Browse'), False, cCard);

  Txt(CustomMessage('GroupShortcuts'), FStrong, cT1, CONTENT_X + 2, 264, W, 20, FmtLeft);
  SwitchCard(HIT_DESKTOP, CONTENT_X, 292, W, GL_MONITOR, CustomMessage('DesktopTitle'), CustomMessage('DesktopDesc'), OptDesktop);
  SwitchCard(HIT_LAUNCH, CONTENT_X, 364, W, GL_PLAY, CustomMessage('LaunchTitle'), CustomMessage('LaunchDesc'), OptLaunch);

  Footer(FmtMessage(CustomMessage('Space'), ['{#AppSizeMB}']), CustomMessage('BtnBack'), CustomMessage('BtnInstall'));
end;

procedure DrawInstalling;
var
  W: Single;
  Pct: String;
begin
  W := CONTENT_R - CONTENT_X;
  if UpdateMode then
    PageHead(CustomMessage('UpdatingTitle'), CustomMessage('UpdatingIntro'))
  else
    PageHead(CustomMessage('InstallingTitle'), CustomMessage('InstallingIntro'));

  Card(CONTENT_X, 148, W, 100, False);
  Glyph(GL_DOWNLOAD, FIcon, cT1, CONTENT_X + 26, 148 + 34, 20);
  Txt(StatusText, FBody, cT1, CONTENT_X + 52, 148 + 14, W - 52 - 80, 20, FmtLeft);
  Txt(FileText, FCaption, cT2, CONTENT_X + 52, 148 + 34, W - 52 - 80, 16, FmtPath);
  Pct := IntToStr(Round(Progress * 100)) + ' %';
  Txt(Pct, FBody, cT2, CONTENT_R - 16 - 80, 148 + 14, 80, 20, FmtRight);
  ProgressBar(CONTENT_X + 52, 148 + 72, W - 52 - 16, Progress);

  if not UpdateMode then
  begin
    Card(CONTENT_X, 256, W, 68, False);
    CardHead(CONTENT_X, 256, W, GL_KEYBOARD, CustomMessage('TipTitle'), CustomMessage('TipDesc'), W - 52 - 16 - 120);
    Shortcut(CONTENT_R - 16, 256 + 24, 'Ctrl+Alt+X');
    Footer('', CustomMessage('BtnCancel'), '');
  end;
end;

procedure DrawDone;
var
  W: Single;
begin
  W := CONTENT_R - CONTENT_X;
  PageHead(CustomMessage('DoneTitle'), '');

  Card(CONTENT_X, 100, W, 88, False);
  FillCircle(CONTENT_X + 40, 100 + 44, 40, cAccentFill);
  Glyph(GL_CHECK, FIconLg, cOnAccent, CONTENT_X + 40, 100 + 44, 24);
  Txt(FmtMessage(CustomMessage('DoneHead'), ['{#AppVersion}']), FSub, cT1, CONTENT_X + 76, 100 + 18, W - 92, 28, FmtLeft);
  Txt(CustomMessage('DoneDesc'), FBody, cT2, CONTENT_X + 76, 100 + 48, W - 92, 20, FmtLeft);

  SwitchCard(HIT_RUNNOW, CONTENT_X, 204, W, GL_PLAY, CustomMessage('RunNowTitle'), CustomMessage('RunNowDesc'), RunNow);

  Footer('', '', CustomMessage('BtnFinish'));
end;

procedure DrawError;
var
  W: Single;
begin
  W := CONTENT_R - CONTENT_X;
  PageHead(CustomMessage('ErrorTitle'), '');
  Card(CONTENT_X, 100, W, 104, False);
  Glyph(GL_ERROR, FIcon, cCritical, CONTENT_X + 26, 100 + 30, 20);
  Txt(ErrorText, FBody, cT1, CONTENT_X + 52, 100 + 20, W - 52 - 16, 72, FmtWrap);
  Footer('', '', CustomMessage('BtnClose'));
end;

{ ContentDialog de WinUI sobre una capa oscura. }
procedure DrawDialog;
var
  X, Y, DW, DH, BW: Single;
  Body: String;
begin
  FillRect(0, 0, WIN_W, WIN_H, Alpha(Hex($000000), 0.3));
  DW := 420;
  DH := 196;
  X := (WIN_W - DW) / 2;
  Y := (WIN_H - DH) / 2;
  { Arriba el contenido; abajo, la franja de botones con el fondo base. }
  FillRound(X, Y, DW, DH, 8, cBg);
  FillRound(X, Y, DW, DH - 80 + 8, 8, cDialog);
  FillRect(X, Y + DH - 80 - 8, DW, 8, cDialog);
  FillRect(X, Y + DH - 80, DW, 1, Over(cCardStroke, cDialog));
  StrokeRound(X, Y, DW, DH, 8, Over(cSurfaceStroke, cBg));
  AddHit(-1, X, Y, DW, DH);

  Txt(CustomMessage('DlgTitle'), FSub, cT1, X + 24, Y + 24, DW - 48, 28, FmtLeft);
  if CurScreen = SCR_INSTALLING then Body := CustomMessage('DlgBodyInstalling') else Body := CustomMessage('DlgBody');
  Txt(Body, FBody, cT1, X + 24, Y + 64, DW - 48, 40, FmtWrap);

  BW := (DW - 48 - 8) / 2;
  Button(HIT_DLG_QUIT, X + 24, Y + DH - 24 - 32, BW, CustomMessage('DlgQuit'), False, cBg);
  Button(HIT_DLG_STAY, X + 24 + BW + 8, Y + DH - 24 - 32, BW, CustomMessage('DlgStay'), True, cBg);
end;

procedure Render;
var
  Bmp: TBitmap;
  PW, PH: Integer;
begin
  if Surface = nil then Exit;
  Bmp := Surface.Bitmap;
  PW := Host.ClientWidth;
  PH := Host.ClientHeight;
  if (Bmp.Width <> PW) or (Bmp.Height <> PH) then
  begin
    Bmp.Width := PW;
    Bmp.Height := PH;
  end;

  if GdipCreateFromHDC(Bmp.Canvas.Handle, G) <> 0 then Exit;
  GdipSetSmoothingMode(G, SMOOTH_ANTIALIAS);
  GdipSetPixelOffsetMode(G, PIXEL_OFFSET_HALF);
  GdipSetTextRenderingHint(G, TEXT_CLEARTYPE);
  GdipSetInterpolationMode(G, INTERP_HQ_BICUBIC);
  GdipGraphicsClear(G, cBg);
  HitCount := 0;

  DrawNav;
  case CurScreen of
    SCR_WELCOME: DrawWelcome;
    SCR_OPTIONS: DrawOptions;
    SCR_INSTALLING: DrawInstalling;
    SCR_DONE: DrawDone;
    SCR_ERROR: DrawError;
  end;
  DrawTitleBar;
  if DialogOpen then DrawDialog;

  GdipDeleteGraphics(G);
  G := 0;

  DirBox.Visible := (CurScreen = SCR_OPTIONS) and not DialogOpen;
  Surface.Invalidate;
end;

{ ── Acciones ─────────────────────────────────────────────────────── }

function WantDesktopIcon: Boolean;
begin
  Result := OptDesktop;
end;

procedure Go(Scr: Integer);
begin
  CurScreen := Scr;
  Hover := HIT_NONE;
  Render;
end;

procedure StartInstall;
begin
  if Trim(DirBox.Text) = '' then
  begin
    Go(SCR_OPTIONS);
    WizardForm.ActiveControl := DirBox;
    Exit;
  end;
  WizardForm.DirEdit.Text := Trim(DirBox.Text);
  RunNow := OptLaunch;
  StatusText := CustomMessage('Preparing');
  FileText := '';
  Progress := 0;
  Go(SCR_INSTALLING);
  if Demo <> '' then Exit;
  WizardForm.NextButton.OnClick(WizardForm.NextButton);
end;

procedure Finish;
begin
  if Demo <> '' then
  begin
    ForceCancel := True;
    WizardForm.Close;
    Exit;
  end;
  if CurScreen = SCR_ERROR then
  begin
    ForceCancel := True;
    WizardForm.CancelButton.OnClick(WizardForm.CancelButton);
    Exit;
  end;
  WizardForm.NextButton.OnClick(WizardForm.NextButton);
end;

procedure AskCancel;
begin
  DialogOpen := True;
  Hover := HIT_NONE;
  Render;
end;

procedure ConfirmCancel;
begin
  DialogOpen := False;
  ForceCancel := True;
  if (Demo <> '') or (CurScreen <> SCR_INSTALLING) then
    WizardForm.Close
  else
    WizardForm.CancelButton.OnClick(WizardForm.CancelButton);
end;

procedure Browse;
var
  Dir: String;
begin
  Dir := DirBox.Text;
  if BrowseForFolder(CustomMessage('BrowsePrompt'), Dir, True) then
  begin
    if CompareText(ExtractFileName(RemoveBackslashUnlessRoot(Dir)), 'Winkit') <> 0 then
      Dir := AddBackslash(Dir) + 'Winkit';
    DirBox.Text := Dir;
  end;
  Render;
end;

procedure Activate(Id: Integer);
begin
  case Id of
    HIT_MIN: SendMessage(WizardForm.Handle, WM_SYSCOMMAND, SC_MINIMIZE, 0);
    HIT_CLOSE:
      if CurScreen = SCR_DONE then Finish
      else if CurScreen = SCR_ERROR then Finish
      else if not DialogOpen then AskCancel;
    HIT_NAV: Go(SCR_WELCOME);
    HIT_NAV + 1: Go(SCR_OPTIONS);
    HIT_PRIMARY:
      case CurScreen of
        SCR_WELCOME, SCR_OPTIONS: StartInstall;
        SCR_DONE, SCR_ERROR: Finish;
      end;
    HIT_SECONDARY:
      case CurScreen of
        SCR_WELCOME: Go(SCR_OPTIONS);
        SCR_OPTIONS: Go(SCR_WELCOME);
        SCR_INSTALLING: AskCancel;
      end;
    HIT_BROWSE: Browse;
    HIT_EDIT: WizardForm.ActiveControl := DirBox;
    HIT_DESKTOP:
      begin
        OptDesktop := not OptDesktop;
        Render;
      end;
    HIT_LAUNCH:
      begin
        OptLaunch := not OptLaunch;
        Render;
      end;
    HIT_RUNNOW:
      begin
        RunNow := not RunNow;
        Render;
      end;
    HIT_DLG_STAY:
      begin
        DialogOpen := False;
        Render;
      end;
    HIT_DLG_QUIT: ConfirmCancel;
  end;
end;

function CursorLogical(var X, Y: Single): Boolean;
var
  P: TPt;
begin
  GetCursorPos(P);
  ScreenToClient(Host.Handle, P);
  X := P.X / S;
  Y := P.Y / S;
  Result := (P.X >= 0) and (P.Y >= 0) and (P.X < Host.ClientWidth) and (P.Y < Host.ClientHeight);
end;

procedure SurfaceClick(Sender: TObject);
var
  X, Y: Single;
  Id: Integer;
begin
  CursorLogical(X, Y);
  Id := HitTest(X, Y);
  if (Id <> HIT_NONE) and ((Id = PressId) or (PressId = HIT_NONE)) then
    Activate(Id);
  PressId := HIT_NONE;
end;

procedure FormKeyDown(Sender: TObject; var Key: Word; Shift: TShiftState);
begin
  if Key = VK_RETURN then
  begin
    Key := 0;
    if DialogOpen then Activate(HIT_DLG_STAY)
    else if CurScreen <> SCR_INSTALLING then Activate(HIT_PRIMARY);
  end
  else if Key = VK_ESCAPE then
  begin
    Key := 0;
    if DialogOpen then Activate(HIT_DLG_STAY)
    else Activate(HIT_CLOSE);
  end;
end;

{ ── Ventana ──────────────────────────────────────────────────────── }

{ Esquinas redondeadas: las de Windows 11 (con sombra y antialias) o, en
  Windows 10, una región recortada. }
procedure ApplyCorners;
var
  Value: Integer;
  Version: TWindowsVersion;
begin
  GetWindowsVersionEx(Version);
  SetWindowLongW(WizardForm.Handle, GWL_STYLE, GetWindowLongW(WizardForm.Handle, GWL_STYLE) or WS_SYSMENU or WS_MINIMIZEBOX);
  if Version.Build >= 22000 then
  begin
    Value := DWMWCP_ROUND;
    DwmSetWindowAttribute(WizardForm.Handle, DWMWA_WINDOW_CORNER_PREFERENCE, Value, 4);
    if Dark then Value := 1 else Value := 0;
    DwmSetWindowAttribute(WizardForm.Handle, DWMWA_USE_IMMERSIVE_DARK_MODE, Value, 4);
    if Dark then Value := $00353535 else Value := $00D8D8D8;
    DwmSetWindowAttribute(WizardForm.Handle, DWMWA_BORDER_COLOR, Value, 4);
  end
  else
    SetWindowRgn(WizardForm.Handle, CreateRoundRectRgn(0, 0, WizardForm.Width + 1, WizardForm.Height + 1, Round(16 * S), Round(16 * S)), 1);
end;

procedure Tick(Wnd, Msg, Id, Time: Longword);
var
  X, Y: Single;
  Inside, NowDown, Changed, Focus: Boolean;
  NewHover: Integer;
begin
  if Ticking then Exit;
  Ticking := True;
  try
    if not CornersDone and WizardForm.Visible then
    begin
      CornersDone := True;
      ApplyCorners;
    end;

    Inside := CursorLogical(X, Y) and (GetForegroundWindow = WizardForm.Handle);
    NowDown := (GetAsyncKeyState(VK_LBUTTON) and $8000) <> 0;
    if Inside then NewHover := HitTest(X, Y) else NewHover := HIT_NONE;

    { Arrastrar desde la barra de título o el hueco de la navegación. }
    if NowDown and not Down and Inside and (NewHover = HIT_NONE) and not DialogOpen and
       ((Y < TITLE_H) or ((X < NAV_W) and (Y < 112))) then
    begin
      ReleaseCapture;
      SendMessage(WizardForm.Handle, WM_NCLBUTTONDOWN, HTCAPTION, 0);
      NowDown := False;
      NewHover := HIT_NONE;
    end;

    if NowDown and not Down then PressId := NewHover;
    Focus := DirBox.Focused;
    Changed := (NewHover <> Hover) or (NowDown <> Down) or (Focus <> EditFocused);
    Hover := NewHover;
    Down := NowDown;
    EditFocused := Focus;
    if Changed then Render;

    { Modo actualización: avanza solo, sin esperar a nadie. }
    if PendingNext then
    begin
      PendingNext := False;
      StartInstall;
    end
    else if PendingFinish then
    begin
      PendingFinish := False;
      Finish;
    end;
  finally
    Ticking := False;
  end;
end;

function HasSwitch(Name: String): Boolean;
var
  I: Integer;
begin
  Result := False;
  for I := 1 to ParamCount do
    if CompareText(ParamStr(I), Name) = 0 then
    begin
      Result := True;
      Exit;
    end;
end;

const
  DemoDir = 'C:\Users\Ana\AppData\Local\Winkit';

procedure BuildUI;
var
  Input: TGdipInput;
begin
  S := ScaleX(100) / 100.0;

  Input.Version := 1;
  Input.Callback := 0;
  Input.NoThread := 0;
  Input.NoCodecs := 0;
  GdiplusStartup(GdipToken, Input, 0);

  LoadTheme;
  LoadFonts;
  ExtractTemporaryFile('winkit-logo.png');
  if GdipLoadImageFromFile(ExpandConstant('{tmp}\winkit-logo.png'), LogoImg) <> 0 then LogoImg := 0;

  WizardForm.BorderStyle := bsNone;
  WizardForm.ClientWidth := Round(WIN_W * S);
  WizardForm.ClientHeight := Round(WIN_H * S);
  WizardForm.Position := poScreenCenter;
  WizardForm.Color := ToTColor(cBg);
  WizardForm.KeyPreview := True;
  WizardForm.OnKeyDown := @FormKeyDown;

  { Una capa que tapa el asistente entero de Inno. }
  Host := TPanel.Create(WizardForm);
  Host.Parent := WizardForm;
  Host.BevelOuter := bvNone;
  Host.ParentBackground := False;
  Host.StyleElements := [];
  Host.Color := ToTColor(cBg);
  Host.SetBounds(0, 0, WizardForm.ClientWidth, WizardForm.ClientHeight);
  Host.BringToFront;

  Surface := TBitmapImage.Create(Host);
  Surface.Parent := Host;
  Surface.SetBounds(0, 0, Host.ClientWidth, Host.ClientHeight);
  Surface.Stretch := False;
  Surface.AutoSize := False;
  Surface.OnClick := @SurfaceClick;

  DirBox := TNewEdit.Create(Host);
  DirBox.Parent := Host;
  DirBox.BorderStyle := bsNone;
  DirBox.StyleElements := [];
  DirBox.Font.Name := 'Segoe UI Variable Text';
  DirBox.Font.Height := -Round(14 * S);
  DirBox.Text := WizardForm.DirEdit.Text;
  DirBox.AutoSelect := False;
  DirBox.Visible := False;

  OptDesktop := True;
  OptLaunch := True;
  RunNow := True;
  CurScreen := SCR_WELCOME;
  Hover := HIT_NONE;
  PressId := HIT_NONE;

  UpdateMode := HasSwitch('/UPDATE') or HasSwitch('/P');
  Relaunch := HasSwitch('/R');

  { Vista previa: una pantalla fija para hacer capturas. }
  Demo := Lowercase(ExpandConstant('{param:DEMO|}'));
  { Ruta genérica en las capturas: que no salga el usuario de quien las hace. }
  if Demo <> '' then DirBox.Text := DemoDir;
  if Demo = 'options' then CurScreen := SCR_OPTIONS
  else if Demo = 'cancel' then DialogOpen := True
  else if Demo = 'installing' then
  begin
    CurScreen := SCR_INSTALLING;
    StatusText := CustomMessage('Copying');
    FileText := DemoDir + '\Winkit.exe';
    Progress := 0.62;
  end
  else if Demo = 'update' then
  begin
    UpdateMode := True;
    CurScreen := SCR_INSTALLING;
    StatusText := CustomMessage('Copying');
    FileText := DemoDir + '\Winkit.exe';
    Progress := 0.38;
  end
  else if Demo = 'done' then CurScreen := SCR_DONE
  else if Demo = 'error' then
  begin
    CurScreen := SCR_ERROR;
    ErrorText := 'Winkit.exe está en uso por otro proceso y no se ha podido reemplazar. Ciérralo desde la bandeja del sistema y vuelve a abrir el instalador.';
  end;

  Render;
  TimerId := SetTimer(0, 0, 16, CreateCallback(@Tick));
end;

{ ── Eventos de Inno ──────────────────────────────────────────────── }

function IsAppRunning: Boolean;
var
  Code: Integer;
begin
  Result := Exec(ExpandConstant('{sys}\cmd.exe'), '/C tasklist /FI "IMAGENAME eq Winkit.exe" | find /I "Winkit.exe"', '', SW_HIDE, ewWaitUntilTerminated, Code) and (Code = 0);
end;

procedure StopApp;
var
  Code: Integer;
begin
  if IsAppRunning then
  begin
    Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM Winkit.exe /T /F', '', SW_HIDE, ewWaitUntilTerminated, Code);
    Sleep(500);
  end;
end;

function WebView2Installed: Boolean;
var
  V: String;
begin
  Result :=
    (RegQueryStringValue(HKLM, 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', V) and (V <> '') and (V <> '0.0.0.0')) or
    (RegQueryStringValue(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', V) and (V <> '') and (V <> '0.0.0.0'));
end;

procedure InitializeWizard;
begin
  MainPage := CreateCustomPage(wpWelcome, '', '');
  if not WizardSilent then
    BuildUI;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  if Surface = nil then Exit;
  if CurPageID = MainPage.ID then
  begin
    if Demo = '' then
    begin
      if UpdateMode then PendingNext := True;
      if CurScreen <> SCR_OPTIONS then CurScreen := SCR_WELCOME;
    end;
  end
  else if CurPageID = wpPreparing then
  begin
    { Si se llega a verse, es que algo ha impedido instalar. }
    ErrorText := WizardForm.PreparingLabel.Caption;
    CurScreen := SCR_ERROR;
  end
  else if CurPageID = wpInstalling then
  begin
    CurScreen := SCR_INSTALLING;
    StatusText := CustomMessage('Copying');
  end
  else if CurPageID = wpFinished then
  begin
    DialogOpen := False;
    CurScreen := SCR_DONE;
    if UpdateMode then
    begin
      RunNow := Relaunch;
      PendingFinish := True;
    end;
  end;
  if Demo = '' then Render;
  if (CurScreen = SCR_OPTIONS) and DirBox.Visible then WizardForm.ActiveControl := DirBox else WizardForm.ActiveControl := Host;
end;

procedure CurInstallProgressChanged(CurProgress, MaxProgress: Integer);
begin
  if Surface = nil then Exit;
  if MaxProgress > 0 then Progress := CurProgress / MaxProgress;
  FileText := WizardForm.FilenameLabel.Caption;
  Render;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
  Bootstrapper: String;
begin
  Result := '';
#ifndef PREVIEW
  StopApp;
  if not WebView2Installed then
  begin
    StatusText := CustomMessage('WebView');
    Render;
    try
      DownloadTemporaryFile('https://go.microsoft.com/fwlink/p/?LinkId=2124703', 'MicrosoftEdgeWebview2Setup.exe', '', nil);
      Bootstrapper := ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe');
      Exec(Bootstrapper, '/silent /install', '', SW_HIDE, ewWaitUntilTerminated, Code);
    except
      Log('WebView2: ' + GetExceptionMessage);
    end;
    StatusText := CustomMessage('Copying');
  end;
#endif
end;

{ El instalador NSIS de antes deja su propia entrada en «Aplicaciones». Al pasar
  a este, se quita para que no salga Winkit dos veces. Los archivos se quedan:
  esta instalación los acaba de sobrescribir en la misma carpeta. }
procedure RemoveNsisLeftovers;
var
  Key: String;
begin
  Key := 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Winkit';
  if RegKeyExists(HKCU, Key) then
  begin
    RegDeleteKeyIncludingSubkeys(HKCU, Key);
    DeleteFile(ExpandConstant('{app}\uninstall.exe'));
  end;
  RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Alejandro Font\Winkit');
  RegDeleteKeyIfEmpty(HKCU, 'Software\Alejandro Font');
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Code: Integer;
begin
#ifndef PREVIEW
  if CurStep = ssPostInstall then
    RemoveNsisLeftovers;
  if (CurStep = ssDone) and (RunNow or (WizardSilent and Relaunch)) then
    ExecAsOriginalUser(ExpandConstant('{app}\Winkit.exe'), '', '', SW_SHOW, ewNoWait, Code);
#endif
end;

procedure CancelButtonClick(CurPageID: Integer; var Cancel, Confirm: Boolean);
begin
  if Surface = nil then Exit;
  if ForceCancel then
  begin
    Confirm := False;
    Cancel := True;
  end
  else
  begin
    { Alt+F4 o el botón oculto de Inno: el aviso es el nuestro. }
    Cancel := False;
    AskCancel;
  end;
end;

procedure DeinitializeSetup;
begin
  if TimerId <> 0 then KillTimer(0, TimerId);
  if GdipToken <> 0 then GdiplusShutdown(GdipToken);
end;

{ ── Desinstalación ───────────────────────────────────────────────── }

function InitializeUninstall: Boolean;
begin
  StopApp;
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  { La app se apunta sola al inicio de Windows (tauri-plugin-autostart). }
  if CurUninstallStep = usPostUninstall then
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Winkit');
end;
