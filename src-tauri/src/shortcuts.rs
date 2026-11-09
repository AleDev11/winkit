//! Atajos globales (RegisterHotKey a través del plugin de Tauri).

use crate::palette::Trigger;
use crate::{launcher, palette, picker, settings, ui};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[derive(Clone, Serialize)]
pub struct ShortcutStatus {
    key: &'static str,
    ok: bool,
}

static MAP: Mutex<Option<HashMap<u32, &'static str>>> = Mutex::new(None);
static STATUS: Mutex<Vec<ShortcutStatus>> = Mutex::new(Vec::new());
static PAUSED: AtomicBool = AtomicBool::new(false);

pub fn status() -> Vec<ShortcutStatus> {
    STATUS.lock().unwrap().clone()
}

pub fn paused() -> bool {
    PAUSED.load(Ordering::Relaxed)
}

pub fn set_paused(p: bool) {
    PAUSED.store(p, Ordering::Relaxed);
    register_all();
}

pub fn unregister_all() {
    let _ = ui::app().global_shortcut().unregister_all();
    *MAP.lock().unwrap() = Some(HashMap::new());
}

pub fn register_all() -> Vec<ShortcutStatus> {
    unregister_all();
    if paused() {
        STATUS.lock().unwrap().clear();
        return Vec::new();
    }
    let s = settings::get().shortcuts;
    let entries: [(&'static str, &str); 7] = [
        ("palette", &s.palette),
        ("correct", &s.correct),
        ("translate", &s.translate),
        ("understand", &s.understand),
        ("launcher", &s.launcher),
        ("colorPicker", &s.color_picker),
        ("saveClip", &s.save_clip),
    ];
    let gs = ui::app().global_shortcut();
    let mut map = HashMap::new();
    let status: Vec<ShortcutStatus> = entries
        .iter()
        .map(|&(key, accel)| {
            let ok = !accel.trim().is_empty()
                && accel.parse::<Shortcut>().is_ok_and(|sc| {
                    let ok = !map.contains_key(&sc.id()) && gs.register(sc).is_ok();
                    if ok {
                        map.insert(sc.id(), key);
                    }
                    ok
                });
            ShortcutStatus { key, ok }
        })
        .collect();
    *MAP.lock().unwrap() = Some(map);
    *STATUS.lock().unwrap() = status.clone();
    status
}

/// Atajos con la tecla asignada que no se han podido registrar (otra app los usa).
pub fn taken() -> Vec<String> {
    let s = settings::get().shortcuts;
    status()
        .iter()
        .filter(|st| !st.ok)
        .filter_map(|st| {
            let accel = match st.key {
                "palette" => &s.palette,
                "correct" => &s.correct,
                "translate" => &s.translate,
                "understand" => &s.understand,
                "launcher" => &s.launcher,
                "saveClip" => &s.save_clip,
                _ => &s.color_picker,
            };
            (!accel.is_empty()).then(|| accel.replace("Control", "Ctrl").replace("Super", "Win").replace('+', " + "))
        })
        .collect()
}

pub fn dispatch(action: &str) {
    match action {
        "palette" => drop(tauri::async_runtime::spawn(palette::trigger(Trigger::Palette))),
        "correct" => drop(tauri::async_runtime::spawn(palette::trigger(Trigger::Correct))),
        "translate" => drop(tauri::async_runtime::spawn(palette::trigger(Trigger::Translate))),
        "understand" => drop(tauri::async_runtime::spawn(palette::trigger(Trigger::Understand))),
        "launcher" => launcher::toggle(),
        "colorPicker" => drop(tauri::async_runtime::spawn(picker::open())),
        "saveClip" => drop(tauri::async_runtime::spawn(save_clip())),
        _ => {}
    }
}

pub fn handler(_app: &AppHandle, sc: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let action = MAP.lock().unwrap().as_ref().and_then(|m| m.get(&sc.id()).copied());
    if let Some(a) = action {
        dispatch(a);
    }
}

/// Atajo de clips: guarda los últimos minutos y avisa.
pub async fn save_clip() {
    let c = settings::get().clips;
    if !c.enabled {
        return palette::notice("Activa los clips en Ajustes para poder guardarlos.", true, None);
    }
    match crate::replay::save().await {
        Ok(_) => palette::notice(format!("Clip guardado: últimos {} min", c.minutes), false, None),
        Err(e) => palette::notice(e, false, None),
    }
}
