//! Cursor, monitores y captura de pantalla. Todo en píxeles físicos (la app es
//! "per-monitor DPI aware").

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    GetMonitorInfoW, MonitorFromPoint, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, MONITORINFO, MONITOR_DEFAULTTONEAREST, SRCCOPY,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl From<RECT> for Rect {
    fn from(r: RECT) -> Self {
        Rect { x: r.left, y: r.top, width: r.right - r.left, height: r.bottom - r.top }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Monitor {
    /// Todo el monitor.
    pub bounds: Rect,
    /// Sin la barra de tareas.
    pub work: Rect,
    /// 1.0 = 96 ppp.
    pub scale: f64,
}

pub fn cursor() -> (i32, i32) {
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

pub fn monitor_at(x: i32, y: i32) -> Monitor {
    unsafe {
        let hmon = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(hmon, &mut info);
        let (mut dx, mut dy) = (96u32, 96u32);
        let _ = GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        Monitor { bounds: info.rcMonitor.into(), work: info.rcWork.into(), scale: dx as f64 / 96.0 }
    }
}

/// Captura un rectángulo de la pantalla. Devuelve RGBA fila a fila.
pub fn capture(r: Rect) -> Option<Vec<u8>> {
    if r.width <= 0 || r.height <= 0 {
        return None;
    }
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, r.width, r.height);
        let old = SelectObject(mem, bmp.into());
        let ok = BitBlt(mem, 0, 0, r.width, r.height, Some(screen), r.x, r.y, SRCCOPY | CAPTUREBLT).is_ok();

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: r.width,
                biHeight: -r.height, // negativo = de arriba abajo
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (r.width * r.height * 4) as usize];
        let lines = if ok {
            GetDIBits(mem, bmp, 0, r.height as u32, Some(pixels.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS)
        } else {
            0
        };

        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        if lines == 0 {
            return None;
        }
        // BGRA → RGBA, opaco.
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        Some(pixels)
    }
}
