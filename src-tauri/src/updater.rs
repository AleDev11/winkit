//! Actualizaciones automáticas desde las releases de GitHub: se comprueba al
//! arrancar y cada pocas horas, se descarga en segundo plano y se instala al
//! reiniciar (desde el aviso, la bandeja o Ajustes). Las releases van firmadas
//! (minisign) y el plugin verifica la firma con la clave pública de tauri.conf.json.

use crate::ui;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::Emitter;
use tauri_plugin_updater::{Update, UpdaterExt};

const FIRST_CHECK: Duration = Duration::from_secs(20);
const INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// "idle" | "checking" | "downloading" | "ready" | "upToDate" | "error"
    state: &'static str,
    version: Option<String>,
    progress: Option<f64>,
    message: Option<String>,
}

struct State {
    status: UpdateStatus,
    /// Descargada y verificada, lista para instalar.
    ready: Option<(Update, Vec<u8>)>,
    busy: bool,
}

static STATE: Mutex<State> = Mutex::new(State {
    status: UpdateStatus { state: "idle", version: None, progress: None, message: None },
    ready: None,
    busy: false,
});

fn set(status: UpdateStatus) {
    STATE.lock().unwrap().status = status.clone();
    let _ = ui::app().emit("update:status", &status);
}

pub fn status() -> UpdateStatus {
    STATE.lock().unwrap().status.clone()
}

pub fn ready_version() -> Option<String> {
    STATE.lock().unwrap().ready.as_ref().map(|(u, _)| u.version.clone())
}

/// Comprueba y, si hay versión nueva, la descarga. Devuelve el estado final.
pub async fn check() -> UpdateStatus {
    {
        let mut st = STATE.lock().unwrap();
        // Ya descargada, o una comprobación en marcha: no repetir.
        if st.ready.is_some() || st.busy {
            return st.status.clone();
        }
        st.busy = true;
    }
    let result = check_inner().await;
    STATE.lock().unwrap().busy = false;
    if let Err(message) = result {
        set(UpdateStatus { state: "error", version: None, progress: None, message: Some(message) });
    }
    status()
}

async fn check_inner() -> Result<(), String> {
    set(UpdateStatus { state: "checking", version: None, progress: None, message: None });
    let updater = ui::app()
        .updater_builder()
        // Al instalar, Windows cierra Winkit con exit(0): que el grabador de clips se pare limpio antes.
        .on_before_exit(crate::replay::shutdown)
        .build()
        .map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|_| "No se ha podido comprobar si hay actualizaciones.".to_string())? else {
        set(UpdateStatus { state: "upToDate", version: None, progress: None, message: None });
        return Ok(());
    };
    let version = update.version.clone();
    let notes = update.body.clone();
    set(UpdateStatus { state: "downloading", version: Some(version.clone()), progress: Some(0.0), message: None });

    let mut received = 0usize;
    let mut last_emit = 0.0f64;
    let v = version.clone();
    let bytes = update
        .download(
            |chunk, total| {
                received += chunk;
                if let Some(total) = total.filter(|t| *t > 0) {
                    let p = received as f64 / total as f64;
                    if p - last_emit >= 0.05 {
                        last_emit = p;
                        set(UpdateStatus { state: "downloading", version: Some(v.clone()), progress: Some(p), message: None });
                    }
                }
            },
            || {},
        )
        .await
        .map_err(|_| "No se ha podido descargar la actualización.".to_string())?;

    STATE.lock().unwrap().ready = Some((update, bytes));
    set(UpdateStatus { state: "ready", version: Some(version.clone()), progress: Some(1.0), message: notes });
    crate::tray_update_ready(&version);
    crate::palette::notice(format!("Winkit {version} está listo. Se instalará al reiniciar."), false, None);
    Ok(())
}

/// Instala la versión descargada. En Windows lanza el instalador (modo pasivo),
/// cierra Winkit y el instalador lo vuelve a abrir al terminar.
pub fn install_and_restart() -> Result<(), String> {
    let ready = STATE.lock().unwrap().ready.take();
    let Some((update, bytes)) = ready else { return Err("No hay ninguna actualización descargada.".into()) };
    update.install(bytes).map_err(|e| e.to_string())
}

/// Bucle en segundo plano. En desarrollo no se comprueba (la versión local no es una release).
pub fn start() {
    if cfg!(debug_assertions) {
        return;
    }
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            check().await;
            if ready_version().is_some() {
                break;
            }
            tokio::time::sleep(INTERVAL).await;
        }
    });
}
