//! Teclado y foco: simular Ctrl+C / Ctrl+V en la app que tiene el foco y
//! saber (y devolver) cuál es esa ventana.

use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

pub const VK_A: u16 = 0x41;
pub const VK_C: u16 = 0x43;
pub const VK_V: u16 = 0x56;
/// Tecla sin asignar: se pulsa antes de soltar Win/Alt a la fuerza para que
/// Windows no abra el menú Inicio ni active la barra de menús de la app.
const VK_NOOP: u16 = 0xE8;

const MODIFIERS: [VIRTUAL_KEY; 5] = [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN];

fn key(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn is_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 }
}

/// El atajo global salta con las teclas aún pulsadas: si mandamos Ctrl+C con Alt
/// abajo, la app recibe Ctrl+Alt+C. Esperamos a que el usuario las suelte y, si
/// tarda demasiado, las soltamos nosotros.
pub async fn wait_for_modifiers_released() {
    let until = Instant::now() + Duration::from_millis(800);
    while Instant::now() < until {
        if !MODIFIERS.iter().any(|&m| is_down(m)) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    let stuck: Vec<VIRTUAL_KEY> = MODIFIERS.iter().copied().filter(|&m| is_down(m)).collect();
    if stuck.is_empty() {
        return;
    }
    let mut inputs = vec![key(VK_NOOP, false), key(VK_NOOP, true)];
    inputs.extend(stuck.iter().map(|m| key(m.0, true)));
    send(&inputs);
}

/// Pulsa Ctrl + `vk` en la ventana que tenga el foco.
pub async fn send_ctrl(vk: u16) {
    send(&[key(VK_CONTROL.0, false), key(vk, false)]);
    tokio::time::sleep(Duration::from_millis(10)).await;
    send(&[key(vk, true), key(VK_CONTROL.0, true)]);
}

pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

pub fn set_foreground_window(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    let h = HWND(hwnd as *mut _);
    unsafe {
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        SetForegroundWindow(h).as_bool()
    }
}
