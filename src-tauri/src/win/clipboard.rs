//! Portapapeles de Win32 a bajo nivel: guardar todo lo que tenga el usuario
//! (texto, HTML, imágenes, archivos…), escribir nuestro texto sin que acabe en
//! el historial de Win+V, y devolverlo todo como estaba.

use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};

const CF_TEXT: u32 = 1;
const CF_OEMTEXT: u32 = 7;
const CF_UNICODETEXT: u32 = 13;
const CF_LOCALE: u32 = 16;
/// Formatos que no son memoria global (GDI, metarchivos, "owner display"): no se pueden copiar como bytes.
const NOT_HGLOBAL: [u32; 8] = [2, 3, 9, 14, 0x80, 0x82, 0x83, 0x8E];
/// Por encima de esto no guardamos el portapapeles (una imagen enorme): mejor perderlo que congelar la app.
const MAX_SNAPSHOT: usize = 64 * 1024 * 1024;

pub struct Snapshot(Vec<(u32, Vec<u8>)>);

/// Abre el portapapeles reintentando: otra app puede tenerlo abierto un instante.
fn open() -> bool {
    let until = Instant::now() + Duration::from_millis(300);
    loop {
        if unsafe { OpenClipboard(None) }.is_ok() {
            return true;
        }
        if Instant::now() > until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

fn read_global(handle: HANDLE) -> Option<Vec<u8>> {
    let g = HGLOBAL(handle.0);
    unsafe {
        let size = GlobalSize(g);
        if size == 0 {
            return Some(Vec::new());
        }
        let ptr = GlobalLock(g) as *const u8;
        if ptr.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
        let _ = GlobalUnlock(g);
        Some(bytes)
    }
}

fn write_global(format: u32, bytes: &[u8]) {
    unsafe {
        let Ok(g) = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) else { return };
        let ptr = GlobalLock(g) as *mut u8;
        if ptr.is_null() {
            return;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(g);
        // Si SetClipboardData funciona, la memoria pasa a ser del sistema.
        let _ = SetClipboardData(format, Some(HANDLE(g.0)));
    }
}

pub fn snapshot() -> Snapshot {
    let mut items = Vec::new();
    if !open() {
        return Snapshot(items);
    }
    let _guard = Guard;
    let has_unicode = unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) }.is_ok();
    let mut total = 0usize;
    let mut format = 0u32;
    loop {
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            break;
        }
        if NOT_HGLOBAL.contains(&format) {
            continue;
        }
        // Windows los genera solo a partir de CF_UNICODETEXT.
        if has_unicode && matches!(format, CF_TEXT | CF_OEMTEXT | CF_LOCALE) {
            continue;
        }
        let Ok(handle) = (unsafe { GetClipboardData(format) }) else { continue };
        if let Some(bytes) = read_global(handle) {
            total += bytes.len();
            if total > MAX_SNAPSHOT {
                return Snapshot(Vec::new());
            }
            items.push((format, bytes));
        }
    }
    Snapshot(items)
}

pub fn restore(s: &Snapshot) {
    if !open() {
        return;
    }
    let _guard = Guard;
    unsafe {
        let _ = EmptyClipboard();
    }
    for (format, bytes) in &s.0 {
        write_global(*format, bytes);
    }
}

pub fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

pub fn read_text() -> Option<String> {
    if !open() {
        return None;
    }
    let _guard = Guard;
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) }.ok()?;
    let bytes = read_global(handle)?;
    let wide: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    Some(String::from_utf16_lossy(&wide[..end]))
}

fn utf16z(text: &str) -> Vec<u8> {
    text.encode_utf16().chain(std::iter::once(0)).flat_map(|c| c.to_le_bytes()).collect()
}

/// Escribe texto. Con `transient`, lo marca para que no entre en el historial
/// del portapapeles ni en la nube (es solo el vehículo para pegar).
pub fn write_text(text: &str, transient: bool) {
    if !open() {
        return;
    }
    let _guard = Guard;
    unsafe {
        let _ = EmptyClipboard();
    }
    write_global(CF_UNICODETEXT, &utf16z(text));
    if transient {
        let zero = 0u32.to_le_bytes();
        unsafe {
            write_global(RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")), &zero);
            write_global(RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")), &zero);
            write_global(RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")), &zero);
        }
    }
}
