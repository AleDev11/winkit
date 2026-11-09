//! Retoques de ventana que Tauri no expone: mostrar sin robar el foco y
//! esquinas redondeadas de Windows 11 en ventanas sin marco.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_SHOWNOACTIVATE,
};

pub fn hwnd_of(w: &impl HasWindowHandle) -> isize {
    match w.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
        _ => 0,
    }
}

fn hwnd(w: &impl HasWindowHandle) -> Option<HWND> {
    match hwnd_of(w) {
        0 => None,
        h => Some(HWND(h as *mut _)),
    }
}

/// Muestra la ventana encima de todo sin quitarle el foco a la app del usuario.
pub fn show_inactive(w: &impl HasWindowHandle) {
    let Some(h) = hwnd(w) else { return };
    unsafe {
        let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

pub fn round_corners(w: &impl HasWindowHandle) {
    let Some(h) = hwnd(w) else { return };
    unsafe {
        let pref = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            h,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of_val(&pref) as u32,
        );
    }
}
