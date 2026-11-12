//! La paleta: todas las herramientas de escritura sobre el texto seleccionado
//! en cualquier app. Lee la selección con Ctrl+C, deja trabajar a la IA y
//! pega el resultado con Ctrl+V, devolviendo el portapapeles como estaba.

use crate::ai::{self, RunEvent, RunRequest};
use crate::lang::{self, LanguageInfo};
use crate::settings::{self, CustomAction};
use crate::ui::{self, PALETTE};
use crate::win::{clipboard, input, screen, window as winx};
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::async_runtime::JoinHandle;
use tauri::ipc::Channel;
use tauri::Emitter;

/// Botón de un aviso. `action`: "openFile" (abrir `arg`), "reveal" (mostrar
/// `arg` en el Explorador), "openSettings", "retryClip" o "installUpdate".
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeAction {
    pub label: String,
    pub action: &'static str,
    pub arg: Option<String>,
    /// El botón principal (color de acento). Uno como mucho.
    pub primary: bool,
}

/// Un aviso de la esquina: «Clip guardado», «#1E90FF copiado», un error…
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    /// "info" | "success" | "progress" | "warning" | "error" | "color"
    pub kind: &'static str,
    pub title: String,
    pub detail: Option<String>,
    /// Muestra de color ("#RRGGBB") en los de tipo "color".
    pub color: Option<String>,
    pub actions: Vec<NoticeAction>,
    /// Cuánto tarda en irse solo (la vista lo cuenta y lo pausa al pasar el
    /// ratón). 0 = no se va solo (los de «en curso»).
    pub duration_ms: u64,
    /// Dónde sale ("bottomRight"…, "cursor"): de ahí entra deslizándose.
    pub placement: String,
    /// Dos avisos seguidos del mismo grupo («Guardando clip…» → «Clip guardado»)
    /// se transforman en el sitio en vez de volver a entrar.
    pub group: Option<String>,
}

impl Notice {
    fn new(kind: &'static str, title: impl Into<String>) -> Self {
        Self { kind, title: title.into(), detail: None, color: None, actions: Vec::new(), duration_ms: 0, placement: String::new(), group: None }
    }
    pub fn info(title: impl Into<String>) -> Self {
        Self::new("info", title)
    }
    pub fn success(title: impl Into<String>) -> Self {
        Self::new("success", title)
    }
    pub fn progress(title: impl Into<String>) -> Self {
        Self::new("progress", title)
    }
    pub fn warning(title: impl Into<String>) -> Self {
        Self::new("warning", title)
    }
    pub fn error(title: impl Into<String>) -> Self {
        Self::new("error", title)
    }
    pub fn color(title: impl Into<String>, hex: impl Into<String>) -> Self {
        Self { color: Some(hex.into()), ..Self::new("color", title) }
    }
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
    pub fn group(mut self, group: &str) -> Self {
        self.group = Some(group.into());
        self
    }
    pub fn action(mut self, label: &str, action: &'static str, arg: Option<String>, primary: bool) -> Self {
        self.actions.push(NoticeAction { label: label.into(), action, arg, primary });
        self
    }

    /// Lo justo para leerlo; más si hay botones o es un error.
    fn auto_duration(&self) -> u64 {
        let chars = (self.title.chars().count() + self.detail.as_ref().map_or(0, |d| d.chars().count())) as u64;
        match self.kind {
            "progress" => 0,
            "color" => 3500,
            "error" | "warning" => (5000 + chars * 40).min(10_000),
            _ if !self.actions.is_empty() => (5000 + chars * 40).min(9000),
            _ => (2500 + chars * 40).min(6000),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestOut {
    action: String,
    text: String,
    language: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteInit {
    session: u64,
    mode: &'static str,
    text: String,
    language: LanguageInfo,
    request: Option<RequestOut>,
    auto_replace: bool,
    can_replace: bool,
    notice: Option<Notice>,
    languages: Vec<String>,
    my_language: String,
    translate_target: String,
    custom_actions: Vec<CustomAction>,
    model_label: String,
}

#[derive(Default)]
struct State {
    init: Option<PaletteInit>,
    session: u64,
    /// Ventana de la que salió el texto (0 = ninguna).
    hwnd: isize,
    /// Cursor al abrir; None = centrada (lector abierto sin selección).
    anchor: Option<(i32, i32)>,
    /// Los avisos van a una esquina del monitor del cursor ("bottomRight"…);
    /// None = se colocan por `anchor`.
    corner: Option<String>,
    /// «Entender»: ventana normal (se mueve y cambia de tamaño); el contenido no la redimensiona.
    free: bool,
    /// El usuario la ha arrastrado: al cambiar de tamaño se queda donde la dejó.
    user_moved: bool,
    focused: bool,
    busy: bool,
    /// Atajo directo con reemplazo automático: se consume con el primer resultado.
    auto_pending: bool,
    task: Option<JoinHandle<()>>,
}

static STATE: Mutex<State> = Mutex::new(State {
    init: None,
    session: 0,
    hwnd: 0,
    anchor: None,
    corner: None,
    free: false,
    user_moved: false,
    focused: false,
    busy: false,
    auto_pending: false,
    task: None,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Clone, Copy, PartialEq)]
pub enum Trigger {
    Palette,
    Correct,
    Translate,
    Understand,
}

pub fn current() -> Option<PaletteInit> {
    state().init.clone()
}

fn palette_hwnd() -> isize {
    ui::get(PALETTE).map(|w| winx::hwnd_of(&w)).unwrap_or(0)
}

/// Oculta la paleta de verdad. `hide()` de Tauri no basta cuando se mostró con
/// `show_inactive` (sin foco): tao solo oculta si cree que estaba visible, y en
/// ese caso no se enteró de que se mostró. Por eso los avisos no se iban.
fn hide_window(w: &tauri::WebviewWindow) {
    let _ = w.hide();
    winx::hide(w);
}

fn is_visible() -> bool {
    ui::get(PALETTE).and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

// ------------------------------------------------------------ captura

struct Capture {
    hwnd: isize,
    text: String,
}

async fn wait_for_copy(before: u32, timeout_ms: u64) -> Option<String> {
    let until = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < until {
        if clipboard::sequence() != before {
            // La app puede anunciar el cambio antes de terminar de escribir.
            tokio::time::sleep(Duration::from_millis(15)).await;
            return clipboard::read_text();
        }
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    None
}

async fn capture_selection(select_all_fallback: bool) -> Capture {
    let hwnd = input::foreground_window();
    input::wait_for_modifiers_released().await;
    let saved = clipboard::snapshot();
    let before = clipboard::sequence();
    input::send_ctrl(input::VK_C).await;
    let mut text = wait_for_copy(before, 450).await.unwrap_or_default();
    if text.trim().is_empty() && select_all_fallback {
        input::send_ctrl(input::VK_A).await;
        tokio::time::sleep(Duration::from_millis(40)).await;
        let before = clipboard::sequence();
        input::send_ctrl(input::VK_C).await;
        text = wait_for_copy(before, 450).await.unwrap_or_default();
    }
    clipboard::restore(&saved);
    Capture { hwnd, text }
}

/// Devuelve el foco a la ventana original y pega `text` sobre la selección.
async fn paste_into(hwnd: isize, text: &str) {
    let saved = clipboard::snapshot();
    clipboard::write_text(text, true);
    if input::foreground_window() != hwnd {
        input::set_foreground_window(hwnd);
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    input::wait_for_modifiers_released().await;
    input::send_ctrl(input::VK_V).await;
    // La app lee el portapapeles de forma asíncrona: no lo devolvemos hasta que haya pegado.
    tokio::time::sleep(Duration::from_millis(350)).await;
    clipboard::restore(&saved);
}

// ------------------------------------------------------------ mostrar / cerrar

fn build_init(mode: &'static str, text: String, request: Option<RequestOut>, auto_replace: bool, can_replace: bool, notice: Option<Notice>) -> PaletteInit {
    let s = settings::get();
    let language = lang::info(&text, &s);
    let mut st = state();
    st.session += 1;
    st.user_moved = false;
    PaletteInit {
        session: st.session,
        mode,
        text,
        language,
        request,
        auto_replace,
        can_replace,
        notice,
        languages: s.languages.clone(),
        my_language: s.my_language.clone(),
        translate_target: s.translate_target.clone(),
        custom_actions: s.custom_actions.clone(),
        model_label: ai::label(&s, &s.ai.for_tool(if mode == "understand" { "understand" } else { "writing" })),
    }
}

fn show(init: PaletteInit, focus: bool) {
    let Some(w) = ui::get(PALETTE) else { return };
    let understand = init.mode == "understand";
    state().init = Some(init.clone());
    let _ = w.emit("palette:init", &init);
    // El lector se comporta como una ventana normal; el resto, como un menú flotante.
    let _ = w.set_always_on_top(!understand);
    if understand {
        set_free(true);
    } else {
        set_free(false);
    }
    // Tamaño provisional hasta que el renderer mida su contenido.
    let (cw, ch) = match init.mode {
        "understand" => (0.0, 0.0),
        "notice" => (320.0, 56.0),
        "run" => (520.0, 160.0),
        _ => (440.0, 320.0),
    };
    if !understand {
        place(cw, ch);
    }
    if focus {
        let _ = w.show();
        let _ = w.set_focus();
    } else {
        // Sin foco: Windows no la subiría por encima de las apps «siempre encima».
        winx::show_inactive(&w);
    }
}

/// Coloca la paleta para un contenido de `w`×`h` px CSS.
pub fn place(w: f64, h: f64) {
    let Some(win) = ui::get(PALETTE) else { return };
    {
        let st = state();
        if st.free {
            return;
        }
        if st.user_moved {
            drop(st);
            return ui::resize_in_place(&win, w, h);
        }
    }
    let (anchor, corner) = { let st = state(); (st.anchor, st.corner.clone()) };
    if let Some(c) = corner {
        return ui::place_corner(&win, screen::cursor(), &c, w, h);
    }
    match anchor {
        Some(a) => ui::place_near(&win, a, w, h),
        None => ui::place_top_center(&win, screen::cursor(), w, h),
    }
}

/// Aviso sencillo de texto. Con `open_settings`, de atención con «Abrir ajustes».
pub fn notice(message: impl Into<String>, open_settings: bool, color: Option<String>) {
    let n = match (color, open_settings) {
        (Some(hex), _) => Notice::color(message, hex),
        (None, true) => Notice::warning(message).action("Abrir ajustes", "openSettings", None, true),
        (None, false) => Notice::info(message),
    };
    show_notice(n);
}

/// Muestra un aviso en la esquina de Ajustes (o junto al ratón), sin quitar el foco.
pub fn show_notice(mut n: Notice) {
    let pos = settings::get().notice_position;
    {
        let mut st = state();
        st.anchor = Some(screen::cursor());
        st.corner = (pos != "cursor").then(|| pos.clone());
        st.hwnd = 0;
    }
    if n.duration_ms == 0 {
        n.duration_ms = n.auto_duration();
    }
    n.placement = pos;
    let duration = n.duration_ms;
    let init = build_init("notice", String::new(), None, false, false, Some(n));
    let session = init.session;
    show(init, false);
    // La vista lo cierra al acabar la cuenta (y la pausa al pasar el ratón);
    // esto es solo por si no llega a hacerlo.
    let safety = if duration == 0 { 60_000 } else { duration + 30_000 };
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(safety)).await;
        if state().session == session {
            close(false);
        }
    });
}

pub fn close(restore_focus: bool) {
    let hwnd = {
        let mut st = state();
        if let Some(t) = st.task.take() {
            t.abort();
        }
        st.focused = false;
        st.auto_pending = false;
        st.session += 1; // invalida temporizadores y respuestas en vuelo
        st.corner = None;
        std::mem::take(&mut st.hwnd)
    };
    let Some(w) = ui::get(PALETTE) else { return };
    if !w.is_visible().unwrap_or(false) {
        return set_free(false);
    }
    remember_free_size(&w);
    if restore_focus && hwnd != 0 {
        input::set_foreground_window(hwnd);
    }
    hide_window(&w);
    set_free(false);
}

pub fn on_focus_change(focused: bool) {
    let close_now = {
        let mut st = state();
        let was = st.focused;
        st.focused = focused;
        // Clic fuera: se cierra como un menú contextual. El lector no.
        !focused && was && st.init.as_ref().is_some_and(|i| i.mode != "understand")
    };
    if close_now {
        close(false);
    }
}

// ------------------------------------------------------------ atajos

/// Lector vacío, centrado, para pegar texto (desde la bandeja o el buscador).
pub fn open_understand_empty() {
    close(false);
    {
        let mut st = state();
        st.anchor = None;
        st.hwnd = 0;
    }
    let init = build_init("understand", String::new(), None, false, false, None);
    show(init, true);
}

pub async fn trigger(kind: Trigger) {
    {
        let mut st = state();
        if st.busy {
            return;
        }
        // El mismo atajo con la paleta abierta la cierra.
        if st.focused && is_visible() {
            drop(st);
            return close(true);
        }
        st.busy = true;
    }
    close(false);
    let anchor = screen::cursor();
    let s = settings::get();
    let cap = capture_selection(s.select_all_fallback && kind != Trigger::Understand).await;
    if cap.hwnd != palette_hwnd() {
        present(kind, cap, anchor, &s);
    }
    state().busy = false;
}

/// Abre la paleta con lo capturado en el modo que pide el atajo.
fn present(kind: Trigger, cap: Capture, anchor: (i32, i32), s: &settings::Settings) {
    let text = cap.text;
    if text.trim().is_empty() {
        return match kind {
            Trigger::Understand => open_understand_empty(),
            _ => show_notice(Notice::info("No hay texto seleccionado").detail("Selecciona un texto en cualquier app y vuelve a pulsar el atajo.")),
        };
    }
    let tool = if kind == Trigger::Understand { "understand" } else { "writing" };
    let provider = s.ai.for_tool(tool).provider;
    if !ai::ready(s, &provider) {
        return notice(ai::not_ready_message(s, &provider), true, None);
    }
    {
        let mut st = state();
        st.hwnd = cap.hwnd;
        st.anchor = if kind == Trigger::Understand { None } else { Some(anchor) };
        st.auto_pending = false;
    }
    match kind {
        Trigger::Palette => show(build_init("menu", text, None, false, true, None), true),
        Trigger::Understand => show(build_init("understand", text, None, false, true, None), true),
        Trigger::Correct | Trigger::Translate => {
            let request = if kind == Trigger::Correct {
                RequestOut { action: "correct".into(), text: text.clone(), language: None }
            } else {
                let to = lang::info(&text, s).translate_to;
                RequestOut { action: "translate".into(), text: text.clone(), language: Some(to) }
            };
            let auto = s.direct_auto_replace;
            state().auto_pending = auto;
            show(build_init("run", text, Some(request), auto, true, None), !auto);
        }
    }
}

// ------------------------------------------------------------ comandos

/// Throttle de los deltas: el texto acumulado viaja entero, así que no más de uno cada 40 ms.
struct Throttle(Mutex<Instant>);
impl Throttle {
    fn ready(&self) -> bool {
        let mut last = self.0.lock().unwrap();
        if last.elapsed() >= Duration::from_millis(40) {
            *last = Instant::now();
            true
        } else {
            false
        }
    }
}

pub fn run(req: RunRequest, ch: Channel<RunEvent>) {
    let s = settings::get();
    let mut req = req;
    let (from, to, fallback) = if ai::is_translation(&req.action) {
        let info = lang::info(&req.text, &s);
        let to = req.language.clone().unwrap_or(if req.action == "translate" {
            info.translate_to.clone()
        } else {
            info.understand_to.clone()
        });
        // Si no se ha podido detectar el idioma, que la IA decida con la regla de respaldo.
        let fallback = info.detected.is_none().then(|| {
            let mine = if req.action == "translate" { &s.translate_target } else { &s.my_language };
            if lang::same(&to, mine) {
                if req.action == "translate" { s.my_language.clone() } else { s.translate_target.clone() }
            } else {
                mine.clone()
            }
        });
        req.language = Some(to.clone());
        (info.detected, Some(to), fallback)
    } else {
        (None, None, None)
    };
    let model = ai::label(&s, &s.ai.for_tool(ai::tool_for_action(&req.action)));
    let _ = ch.send(RunEvent::Start { model, from, to });

    let mut st = state();
    if let Some(t) = st.task.take() {
        t.abort();
    }
    let session = st.session;
    st.task = Some(tauri::async_runtime::spawn(async move {
        let throttle = Throttle(Mutex::new(Instant::now() - Duration::from_secs(1)));
        let delta_ch = ch.clone();
        let on_delta = move |t: &str| {
            if throttle.ready() {
                let _ = delta_ch.send(RunEvent::Delta { text: t.to_string() });
            }
        };
        let result = ai::run(&s, &req, fallback.as_deref(), &on_delta).await;
        let (auto, hwnd, focused) = {
            let mut st = state();
            if st.session != session {
                return;
            }
            (std::mem::take(&mut st.auto_pending), st.hwnd, st.focused)
        };
        match result {
            Ok(text) => {
                let _ = ch.send(RunEvent::Done { text: text.clone() });
                // Solo pegamos a ciegas si el usuario sigue en la ventana de la que salió el texto.
                if auto && hwnd != 0 && input::foreground_window() == hwnd {
                    return replace(text).await;
                }
            }
            Err(f) => {
                let _ = ch.send(RunEvent::Error { message: f.message, open_settings: f.open_settings });
            }
        }
        if !focused {
            if let Some(w) = ui::get(PALETTE) {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
    }));
}

pub fn cancel() {
    if let Some(t) = state().task.take() {
        t.abort();
    }
}

pub async fn replace(text: String) {
    let hwnd = {
        let mut st = state();
        st.focused = false;
        st.auto_pending = false;
        st.session += 1;
        std::mem::take(&mut st.hwnd)
    };
    if hwnd == 0 {
        return;
    }
    // Primero el foco a la app original y luego ocultar: así Windows no lo da a otra ventana.
    input::set_foreground_window(hwnd);
    if let Some(w) = ui::get(PALETTE) {
        hide_window(&w);
    }
    paste_into(hwnd, &text).await;
}

// ------------------------------------------------------------ mover y redimensionar

/// «Entender» se comporta como una ventana normal: se mueve, cambia de tamaño
/// (con un mínimo) y recuerda el tamaño. El resto de vistas son menús que
/// miden lo que su contenido. La vista lo pide al cambiar de modo.
pub fn set_free(free: bool) {
    let Some(w) = ui::get(PALETTE) else { return };
    {
        let mut st = state();
        if st.free == free {
            return;
        }
        st.free = free;
    }
    let _ = w.set_resizable(free);
    if free {
        let _ = w.set_min_size(Some(tauri::LogicalSize::new(MIN_FREE.0, MIN_FREE.1)));
        let size = settings::get().understand_size;
        ui::place_center(&w, screen::cursor(), size.width.max(MIN_FREE.0), size.height.max(MIN_FREE.1));
    } else {
        let _ = w.set_min_size(None::<tauri::LogicalSize<f64>>);
    }
}

const MIN_FREE: (f64, f64) = (440.0, 320.0);

/// Arrastrar la ventana desde la cabecera. A partir de aquí, si el contenido
/// cambia de tamaño, se queda donde el usuario la ha dejado.
pub fn drag() {
    let Some(w) = ui::get(PALETTE) else { return };
    state().user_moved = true;
    let _ = w.start_dragging();
}

/// Guarda el tamaño de «Entender» para la próxima vez.
fn remember_free_size(w: &tauri::WebviewWindow) {
    if !state().free {
        return;
    }
    let (Ok(size), Ok(scale)) = (w.inner_size(), w.scale_factor()) else { return };
    let size = size.to_logical::<f64>(scale);
    let saved = settings::get().understand_size;
    if (saved.width - size.width).abs() < 1.0 && (saved.height - size.height).abs() < 1.0 {
        return;
    }
    let _ = settings::save_patch(serde_json::json!({
        "understandSize": { "width": size.width.round(), "height": size.height.round() }
    }));
}
