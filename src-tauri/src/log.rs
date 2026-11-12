//! Registro en %LOCALAPPDATA%\Winkit\winkit.log. La versión instalada no tiene
//! consola, así que sin esto un fallo (p. ej. del grabador de clips) no deja
//! rastro. Nunca se escribe texto del usuario ni la API key: solo eventos.
//!
//! Uso: `crate::log::info("clips", "arrancado con NVENC")`, o las macros
//! `log_info!` / `log_warn!` / `log_error!` con formato.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Al pasar de este tamaño, el registro actual pasa a winkit.old.log.
const MAX_BYTES: u64 = 1024 * 1024;

static LOCK: Mutex<()> = Mutex::new(());

pub fn path() -> PathBuf {
    // Los tests no ensucian el registro de la app instalada.
    if cfg!(test) {
        return std::env::temp_dir().join("winkit-test.log");
    }
    let base = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    base.join("Winkit").join("winkit.log")
}

/// "2026-11-12 14:26:22" en hora UTC (sin dependencias de zona horaria).
fn timestamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Días desde 1970 → fecha civil (algoritmo de Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

pub fn write(level: &str, area: &str, msg: &str) {
    let line = format!("{} {level:<5} [{area}] {msg}\n", timestamp());
    if cfg!(debug_assertions) {
        eprint!("{line}");
    }
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(&p).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(&p, p.with_file_name("winkit.old.log"));
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&p) {
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn info(area: &str, msg: &str) {
    write("INFO", area, msg);
}

pub fn warn(area: &str, msg: &str) {
    write("WARN", area, msg);
}

pub fn error(area: &str, msg: &str) {
    write("ERROR", area, msg);
}

#[macro_export]
macro_rules! log_info {
    ($area:expr, $($arg:tt)*) => { $crate::log::info($area, &format!($($arg)*)) };
}

#[macro_export]
macro_rules! log_warn {
    ($area:expr, $($arg:tt)*) => { $crate::log::warn($area, &format!($($arg)*)) };
}

#[macro_export]
macro_rules! log_error {
    ($area:expr, $($arg:tt)*) => { $crate::log::error($area, &format!($($arg)*)) };
}

/// Que un panic quede escrito antes de que el proceso muera (release usa panic = "abort").
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        error("panic", &format!("hilo «{thread}»: {info}"));
        prev(info);
    }));
}

#[cfg(test)]
mod tests {
    #[test]
    fn fecha_iso() {
        let t = super::timestamp();
        assert_eq!(t.len(), 20, "{t}");
        assert!(t.starts_with("20") && t.ends_with('Z'), "{t}");
    }
}
