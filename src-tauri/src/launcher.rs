//! El buscador (Alt+Espacio): barra flotante centrada, como Spotlight.

use crate::claude::{self, RunEvent};
use crate::settings;
use crate::ui::{self, LAUNCHER};
use crate::win::{apps, screen};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::async_runtime::JoinHandle;
use tauri::ipc::Channel;
use tauri::Emitter;

const WIDTH: f64 = 680.0;
const APPS_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherInit {
    session: u64,
    engine_label: String,
    model_label: String,
}

struct State {
    init: Option<LauncherInit>,
    session: u64,
    /// Punto de referencia del monitor donde se abrió.
    monitor: (i32, i32),
    task: Option<JoinHandle<()>>,
}

static STATE: Mutex<State> = Mutex::new(State { init: None, session: 0, monitor: (0, 0), task: None });

struct AppsCache {
    list: Vec<apps::AppEntry>,
    built: Option<Instant>,
    building: bool,
}

static APPS: Mutex<AppsCache> = Mutex::new(AppsCache { list: Vec::new(), built: None, building: false });
static ICONS: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn current() -> Option<LauncherInit> {
    state().init.clone()
}

pub fn hide() {
    if let Some(t) = state().task.take() {
        t.abort();
    }
    if let Some(w) = ui::get(LAUNCHER) {
        let _ = w.hide();
    }
}

pub fn toggle() {
    let Some(w) = ui::get(LAUNCHER) else { return };
    if w.is_visible().unwrap_or(false) {
        return hide();
    }
    let s = settings::get();
    let init = {
        let mut st = state();
        st.session += 1;
        st.monitor = screen::cursor();
        let init = LauncherInit { session: st.session, engine_label: s.engine_label().into(), model_label: s.model_label() };
        st.init = Some(init.clone());
        init
    };
    let _ = w.emit("launcher:init", &init);
    resize(WIDTH, 64.0);
    let _ = w.show();
    let _ = w.set_focus();
}

pub fn resize(_width: f64, height: f64) {
    let Some(w) = ui::get(LAUNCHER) else { return };
    let monitor = state().monitor;
    ui::place_top_center(&w, monitor, WIDTH, height);
}

pub fn ask(question: String, ch: Channel<RunEvent>) {
    let s = settings::get();
    let _ = ch.send(RunEvent::Start { model: s.model_label(), from: None, to: None });
    let mut st = state();
    if let Some(t) = st.task.take() {
        t.abort();
    }
    st.task = Some(tauri::async_runtime::spawn(async move {
        let last = Mutex::new(Instant::now() - Duration::from_secs(1));
        let delta_ch = ch.clone();
        let on_delta = move |t: &str| {
            let mut l = last.lock().unwrap();
            if l.elapsed() >= Duration::from_millis(40) {
                *l = Instant::now();
                let _ = delta_ch.send(RunEvent::Delta { text: t.to_string() });
            }
        };
        let _ = match claude::ask(&s, &question, &on_delta).await {
            Ok(text) => ch.send(RunEvent::Done { text }),
            Err(f) => ch.send(RunEvent::Error { message: f.message, open_settings: f.open_settings }),
        };
    }));
}

// ------------------------------------------------------------ apps

async fn rebuild() {
    let list = tauri::async_runtime::spawn_blocking(apps::list).await.unwrap_or_default();
    let mut c = APPS.lock().unwrap();
    if !list.is_empty() || c.list.is_empty() {
        c.list = list;
    }
    c.built = Some(Instant::now());
    c.building = false;
}

/// El índice al instante; si está viejo, se renueva en segundo plano.
pub async fn list_apps() -> Vec<apps::AppEntry> {
    let (stale, empty) = {
        let mut c = APPS.lock().unwrap();
        let stale = c.built.is_none_or(|b| b.elapsed() > APPS_TTL) && !c.building;
        if stale {
            c.building = true;
        }
        (stale, c.list.is_empty())
    };
    if stale {
        if empty {
            rebuild().await;
        } else {
            tauri::async_runtime::spawn(rebuild());
        }
    }
    APPS.lock().unwrap().list.clone()
}

pub fn warm_up() {
    tauri::async_runtime::spawn(async {
        list_apps().await;
    });
}

pub async fn icon(id: String) -> Option<String> {
    if let Some(hit) = ICONS.lock().unwrap().get_or_insert_with(HashMap::new).get(&id) {
        return hit.clone();
    }
    let key = id.clone();
    let url = tauri::async_runtime::spawn_blocking(move || apps::icon(&key, 64)).await.ok().flatten();
    ICONS.lock().unwrap().get_or_insert_with(HashMap::new).insert(id, url.clone());
    url
}
