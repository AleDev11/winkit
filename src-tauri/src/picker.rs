//! Selector de color: congela el monitor bajo el cursor al pulsar el atajo y
//! abre encima una ventana a pantalla completa con la captura y una lupa.

use crate::settings;
use crate::ui::{self, PICKER};
use crate::win::{clipboard, screen};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Emitter, WebviewUrl, WebviewWindowBuilder};

#[derive(Clone, Serialize)]
pub struct PickerInit {
    session: u64,
}

struct Shot {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

struct State {
    shot: Option<Shot>,
    session: u64,
    busy: bool,
}

static STATE: Mutex<State> = Mutex::new(State { shot: None, session: 0, busy: false });

#[derive(Deserialize)]
pub struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

pub fn current() -> Option<PickerInit> {
    let st = STATE.lock().unwrap();
    st.shot.as_ref().map(|_| PickerInit { session: st.session })
}

pub async fn open() {
    {
        let mut st = STATE.lock().unwrap();
        if st.busy || st.shot.is_some() {
            return;
        }
        st.busy = true;
    }
    crate::palette::close(false);
    crate::launcher::hide();
    let (cx, cy) = screen::cursor();
    let mon = screen::monitor_at(cx, cy);
    let b = mon.bounds;
    let Some(rgba) = screen::capture(b) else {
        STATE.lock().unwrap().busy = false;
        return;
    };
    let init = {
        let mut st = STATE.lock().unwrap();
        st.session += 1;
        st.shot = Some(Shot { width: b.width as u32, height: b.height as u32, rgba });
        st.busy = false;
        PickerInit { session: st.session }
    };

    let win = match ui::get(PICKER) {
        Some(w) => w,
        None => {
            let built = WebviewWindowBuilder::new(ui::app(), PICKER, WebviewUrl::App("windows/picker/index.html".into()))
                .title("Winkit · Color")
                .decorations(false)
                .resizable(false)
                .skip_taskbar(true)
                .always_on_top(true)
                .shadow(false)
                .visible(false)
                .build();
            match built {
                Ok(w) => w,
                Err(_) => {
                    STATE.lock().unwrap().shot = None;
                    return;
                }
            }
        }
    };
    ui::set_bounds(&win, b.x, b.y, b.width, b.height);
    let _ = win.emit("picker:init", &init);
    let _ = win.show();
    let _ = win.set_focus();
}

/// Ancho y alto (u32 LE) seguidos de los píxeles RGBA.
pub fn capture_bytes() -> Vec<u8> {
    let st = STATE.lock().unwrap();
    let Some(shot) = st.shot.as_ref() else { return vec![0; 8] };
    let mut out = Vec::with_capacity(8 + shot.rgba.len());
    out.extend_from_slice(&shot.width.to_le_bytes());
    out.extend_from_slice(&shot.height.to_le_bytes());
    out.extend_from_slice(&shot.rgba);
    out
}

pub fn format(c: &Rgb, fmt: &str) -> String {
    match fmt {
        "rgb" => format!("rgb({}, {}, {})", c.r, c.g, c.b),
        "hsl" => {
            let (r, g, b) = (c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0);
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let l = (max + min) / 2.0;
            let d = max - min;
            let (h, s) = if d == 0.0 {
                (0.0, 0.0)
            } else {
                let s = d / (1.0 - (2.0 * l - 1.0).abs());
                let h = if max == r {
                    60.0 * (((g - b) / d).rem_euclid(6.0))
                } else if max == g {
                    60.0 * ((b - r) / d + 2.0)
                } else {
                    60.0 * ((r - g) / d + 4.0)
                };
                (h, s)
            };
            format!("hsl({}, {}%, {}%)", h.round() as i32 % 360, (s * 100.0).round(), (l * 100.0).round())
        }
        _ => format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b),
    }
}

pub fn done(color: Option<Rgb>) {
    STATE.lock().unwrap().shot = None;
    // Se destruye: una webview a pantalla completa no merece quedarse en memoria.
    if let Some(w) = ui::get(PICKER) {
        let _ = w.destroy();
    }
    let Some(c) = color else { return };
    let text = format(&c, &settings::get().color_format);
    clipboard::write_text(&text, false);
    let hex = format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b);
    crate::palette::notice(format!("{text} copiado"), false, Some(hex));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatos() {
        let c = Rgb { r: 30, g: 144, b: 255 };
        assert_eq!(format(&c, "hex"), "#1E90FF");
        assert_eq!(format(&c, "rgb"), "rgb(30, 144, 255)");
        assert_eq!(format(&c, "hsl"), "hsl(210, 100%, 56%)");
    }
}
