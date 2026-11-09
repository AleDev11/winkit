//! Lo que el sistema expone por registro y shell: versión de Windows, color de
//! acento, abrir cosas y los comandos del buscador.

use std::os::windows::process::CommandExt;
use std::process::Command;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Registry::{
    RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Pictures, SHEmptyRecycleBinW,
    SHGetKnownFolderPath, ShellExecuteW, KF_FLAG_DEFAULT, SHERB_NOCONFIRMATION, SHERB_NOSOUND,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn reg_dword(root: HKEY, key: &str, value: &str) -> Option<u32> {
    let mut data = 0u32;
    let mut size = 4u32;
    unsafe {
        RegGetValueW(
            root,
            &HSTRING::from(key),
            &HSTRING::from(value),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut _),
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    Some(data)
}

fn reg_string(root: HKEY, key: &str, value: &str) -> Option<String> {
    let mut buf = [0u16; 256];
    let mut size = (buf.len() * 2) as u32;
    unsafe {
        RegGetValueW(
            root,
            &HSTRING::from(key),
            &HSTRING::from(value),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// Windows 11 (build 22000 o superior): hay Mica y esquinas redondeadas.
pub fn is_windows_11() -> bool {
    reg_string(HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "CurrentBuildNumber")
        .and_then(|b| b.parse::<u32>().ok())
        .is_some_and(|b| b >= 22000)
}

/// Color de acento del usuario como "#rrggbb".
pub fn accent_color() -> String {
    // El registro lo guarda como 0xAABBGGRR.
    match reg_dword(HKEY_CURRENT_USER, r"Software\Microsoft\Windows\DWM", "AccentColor") {
        Some(c) => format!("#{:02x}{:02x}{:02x}", c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff),
        None => "#0067c0".into(),
    }
}

/// Abre un archivo, carpeta, URL o URI (`ms-settings:`) con el programa predeterminado.
pub fn open(target: &str) {
    unsafe {
        ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(target), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
    }
}

/// Abre una app del menú Inicio (o cualquier elemento de shell:AppsFolder) por su nombre de análisis.
pub fn open_app(id: &str) {
    let _ = Command::new("explorer.exe").arg(format!("shell:AppsFolder\\{id}")).spawn();
}

fn known_folder(id: &windows::core::GUID) -> Option<String> {
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

fn shutdown(args: &[&str]) {
    let _ = Command::new("shutdown.exe").args(args).creation_flags(CREATE_NO_WINDOW).spawn();
}

/// Los comandos del buscador que resuelve el sistema (ver src/shared/commands.ts).
/// Devuelve false si el id no es de los que se resuelven aquí.
pub fn run_command(id: &str) -> bool {
    match id {
        "lock" => unsafe {
            let _ = LockWorkStation();
        },
        "sleep" => unsafe {
            let _ = SetSuspendState(false, false, false);
        },
        "shutdown" => shutdown(&["/s", "/t", "0"]),
        "restart" => shutdown(&["/r", "/t", "0"]),
        "signout" => shutdown(&["/l"]),
        "emptyRecycleBin" => unsafe {
            let _ = SHEmptyRecycleBinW(None, PCWSTR::null(), SHERB_NOCONFIRMATION | SHERB_NOSOUND);
        },
        "recycleBin" => open("shell:RecycleBinFolder"),
        "folder:downloads" => known_folder(&FOLDERID_Downloads).iter().for_each(|p| open(p)),
        "folder:documents" => known_folder(&FOLDERID_Documents).iter().for_each(|p| open(p)),
        "folder:desktop" => known_folder(&FOLDERID_Desktop).iter().for_each(|p| open(p)),
        "folder:pictures" => known_folder(&FOLDERID_Pictures).iter().for_each(|p| open(p)),
        "folder:appdata" => std::env::var("APPDATA").iter().for_each(|p| open(p)),
        "folder:temp" => open(&std::env::temp_dir().to_string_lossy()),
        "settings" => open("ms-settings:"),
        "taskmgr" => open("taskmgr.exe"),
        "controlPanel" => open("control.exe"),
        _ => return false,
    }
    true
}
