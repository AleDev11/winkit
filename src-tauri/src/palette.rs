//! La paleta: todas las herramientas de escritura sobre el texto seleccionado
//! en cualquier app. Lee la selección con Ctrl+C, deja trabajar a Claude y
//! pega el resultado con Ctrl+V, devolviendo el portapapeles como estaba.

use crate::claude::{self, RunEvent, RunRequest};
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub message: String,
    pub open_settings: bool,
    pub color: Option<String>,
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
        model_label: s.model_label(),
    }
}

fn show(init: PaletteInit, focus: bool) {
    let Some(w) = ui::get(PALETTE) else { return };
    let understand = init.mode == "understand";
    state().init = Some(init.clone());
    let _ = w.emit("palette:init", &init);
    // El lector se comporta como una ventana normal; el resto, como un menú flotante.
    let _ = w.set_always_on_top(!understand);
    // Tamaño provisional hasta que el renderer mida su contenido.
    let (cw, ch) = match init.mode {
        "understand" => (880.0, 540.0),
        "notice" => (320.0, 56.0),
        "run" => (520.0, 160.0),
        _ => (440.0, 320.0),
    };
    place(cw, ch);
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
    let anchor = state().anchor;
    match anchor {
        Some(a) => ui::place_near(&win, a, w, h),
        None => ui::place_top_center(&win, screen::cursor(), w, h),
    }
}

pub fn notice(message: impl Into<String>, open_settings: bool, color: Option<String>) {
    {
        let mut st = state();
        st.anchor = Some(screen::cursor());
        st.hwnd = 0;
    }
    let init = build_init("notice", String::new(), None, false, false, Some(Notice { message: message.into(), open_settings, color }));
    let session = init.session;
    show(init, open_settings);
    if !open_settings {
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(2200)).await;
            if state().session == session {
                close(false);
            }
        });
    }
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
        std::mem::take(&mut st.hwnd)
    };
    let Some(w) = ui::get(PALETTE) else { return };
    if !w.is_visible().unwrap_or(false) {
        return;
    }
    if restore_focus && hwnd != 0 {
        input::set_foreground_window(hwnd);
    }
    let _ = w.hide();
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
            _ => notice("No hay texto seleccionado.", false, None),
        };
    }
    if settings::api_key().is_none() {
        return notice("Añade tu API key de Anthropic para empezar.", true, None);
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
    let (from, to, fallback) = if claude::is_translation(&req.action) {
        let info = lang::info(&req.text, &s);
        let to = req.language.clone().unwrap_or(if req.action == "translate" {
            info.translate_to.clone()
        } else {
            info.understand_to.clone()
        });
        // Si no se ha podido detectar el idioma, que Claude decida con la regla de respaldo.
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
    let _ = ch.send(RunEvent::Start { model: s.model_label(), from, to });

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
        let result = claude::run(&s, &req, fallback.as_deref(), &on_delta).await;
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
        let _ = w.hide();
    }
    paste_into(hwnd, &text).await;
}
