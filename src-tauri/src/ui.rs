//! Creación y colocación de ventanas. La paleta y el buscador se crean al
//! arrancar (tienen que salir al instante) y se reutilizan; Ajustes y el
//! selector de color se crean al abrirlos y se destruyen al cerrarlos.

use crate::win::{screen, system, window as winx};
use serde::Serialize;
use std::sync::OnceLock;
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const PALETTE: &str = "palette";
pub const LAUNCHER: &str = "launcher";
pub const SETTINGS: &str = "settings";
pub const PICKER: &str = "picker";

static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn set_app(app: AppHandle) {
    let _ = APP.set(app);
}

pub fn app() -> &'static AppHandle {
    APP.get().expect("la app aún no está lista")
}

pub fn get(label: &str) -> Option<WebviewWindow> {
    app().get_webview_window(label)
}

static WIN11: OnceLock<bool> = OnceLock::new();

pub fn win11() -> bool {
    *WIN11.get_or_init(system::is_windows_11)
}

#[derive(Serialize)]
pub struct UiEnv {
    backdrop: &'static str,
    accent: String,
}

pub fn env() -> UiEnv {
    UiEnv { backdrop: if win11() { "acrylic" } else { "none" }, accent: system::accent_color() }
}

fn page(name: &str) -> WebviewUrl {
    WebviewUrl::App(format!("windows/{name}/index.html").into())
}

/// Ventana flotante sin marco: translúcida (acrílico) en Windows 11, con la
/// sombra y las esquinas redondeadas del sistema.
fn popup(app: &AppHandle, label: &str, width: f64, height: f64) -> tauri::Result<WebviewWindow> {
    let mut b = WebviewWindowBuilder::new(app, label, page(label))
        .title("Winkit")
        .inner_size(width, height)
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .focused(false)
        .shadow(true);
    if win11() {
        b = b.transparent(true).effects(EffectsBuilder::new().effect(Effect::Acrylic).build());
    }
    let w = b.build()?;
    winx::round_corners(&w);
    Ok(w)
}

pub fn create_popups(app: &AppHandle) -> tauri::Result<()> {
    popup(app, PALETTE, 440.0, 240.0)?;
    popup(app, LAUNCHER, 680.0, 64.0)?;
    Ok(())
}

pub fn open_settings() {
    if let Some(w) = get(SETTINGS) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let mut b = WebviewWindowBuilder::new(app(), SETTINGS, page(SETTINGS))
        .title("Winkit")
        .inner_size(860.0, 660.0)
        .min_inner_size(680.0, 480.0)
        .center()
        .visible(true)
        .focused(true);
    if win11() {
        b = b.transparent(true).effects(EffectsBuilder::new().effect(Effect::Mica).build());
    }
    if let Ok(w) = b.build() {
        w.on_window_event(|e| {
            if let tauri::WindowEvent::Destroyed = e {
                // Por si se cerró mientras se grababa un atajo.
                crate::shortcuts::register_all();
            }
        });
        let _ = w.set_focus();
    }
}

/// Mueve y redimensiona en píxeles físicos. La posición se aplica antes y
/// después: si la ventana cambia de monitor (y de escala), Windows la reescala.
pub fn set_bounds(w: &WebviewWindow, x: i32, y: i32, width: i32, height: i32) {
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.set_size(PhysicalSize::new(width.max(1) as u32, height.max(1) as u32));
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

/// Coloca una ventana de `w`×`h` px CSS junto al punto (abajo a la derecha; si
/// no cabe, arriba o a la izquierda), dentro del área de trabajo.
pub fn place_near(win: &WebviewWindow, anchor: (i32, i32), w: f64, h: f64) {
    let mon = screen::monitor_at(anchor.0, anchor.1);
    let (pw, ph) = ((w * mon.scale).ceil() as i32, (h * mon.scale).ceil() as i32);
    let gap = (14.0 * mon.scale) as i32;
    let wa = mon.work;
    let mut x = anchor.0 + gap;
    let mut y = anchor.1 + gap;
    if x + pw > wa.x + wa.width {
        x = anchor.0 - gap - pw;
    }
    if y + ph > wa.y + wa.height {
        y = anchor.1 - gap - ph;
    }
    x = x.clamp(wa.x, (wa.x + wa.width - pw).max(wa.x));
    y = y.clamp(wa.y, (wa.y + wa.height - ph).max(wa.y));
    set_bounds(win, x, y, pw, ph);
}

/// Centrada en horizontal con el borde superior fijo (a ~1/5 del alto), para
/// que no salte al crecer. `monitor_point` decide el monitor.
pub fn place_top_center(win: &WebviewWindow, monitor_point: (i32, i32), w: f64, h: f64) {
    let mon = screen::monitor_at(monitor_point.0, monitor_point.1);
    let (pw, ph) = ((w * mon.scale).ceil() as i32, (h * mon.scale).ceil() as i32);
    let wa = mon.work;
    let x = wa.x + (wa.width - pw) / 2;
    let mut y = wa.y + wa.height / 5;
    if y + ph > wa.y + wa.height {
        y = (wa.y + wa.height - ph).max(wa.y);
    }
    set_bounds(win, x, y, pw, ph);
}
